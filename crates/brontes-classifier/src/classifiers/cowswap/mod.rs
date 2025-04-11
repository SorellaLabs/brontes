use alloy_primitives::{Address, U256};
use brontes_database::libmdbx::{DBWriter, LibmdbxReader};
use brontes_macros::action_impl;
use brontes_pricing::Protocol;
use brontes_types::{
    normalized_actions::{NormalizedBatch, NormalizedSwap},
    structured_trace::CallInfo,
    ToScaledRational,
};
use eyre::Error;
use Protocol::Cowswap;

use crate::cow_swap_bindings::{CowswapGPv2Settlement, CowswapGPv2Settlement::Trade};

fn create_normalized_swap<DB: LibmdbxReader + DBWriter>(
    trade: &Trade,
    db_tx: &DB,
    protocol: Protocol,
    pool_address: Address,
    trace_index: u64,
) -> Result<NormalizedSwap, Error> {
    let token_in_info = db_tx.try_fetch_token_info(trade.sellToken)?;
    let token_out_info = db_tx.try_fetch_token_info(trade.buyToken)?;

    let amount_in = trade.sellAmount.to_scaled_rational(token_in_info.decimals);
    let amount_out = trade.buyAmount.to_scaled_rational(token_out_info.decimals);

    Ok(NormalizedSwap {
        protocol,
        trace_index,
        from: trade.owner,
        recipient: trade.owner,
        pool: pool_address,
        token_in: token_in_info,
        token_out: token_out_info,
        amount_in,
        amount_out,
        msg_value: U256::ZERO,
    })
}

action_impl!(
    Protocol::Cowswap,
    CowswapGPv2Settlement::swapCall,
    Batch,
    [..Trade],
    call_data: true,
    logs: true,
    |info: CallInfo, _call_data: swapCall, log_data: CowswapSwapCallLogs, db_tx: &DB| {
        let tx_to = info.target_address;
        let trade_logs = log_data.trade_field?;
        let swap = create_normalized_swap(&trade_logs, db_tx, Cowswap, tx_to, 0)?;

        Ok(NormalizedBatch {
            protocol: Cowswap,
            trace_index: info.trace_idx,
            solver: info.msg_sender,
            settlement_contract: tx_to,
            user_swaps: vec![swap],
            solver_swaps: None,
            msg_value: info.msg_value,
        })
    }
);

action_impl!(
    Protocol::Cowswap,
    CowswapGPv2Settlement::settleCall,
    Batch,
    [..Trade*],
    call_data: true,
    logs: true,
    |info: CallInfo, _call_data: settleCall, log_data: CowswapSettleCallLogs, db_tx: &DB| {
        let trade_logs = log_data.trade_field?;
        let user_swaps: Result<Vec<NormalizedSwap>, Error> = trade_logs.iter().map(|trade| {
            create_normalized_swap(trade, db_tx, Protocol::Cowswap, info.target_address, 0)
        }).collect();

        let user_swaps = user_swaps?;

        Ok(NormalizedBatch {
            protocol: Protocol::Cowswap,
            trace_index: info.trace_idx,
            solver: info.msg_sender,
            settlement_contract: info.target_address,
            user_swaps,
            solver_swaps: None,
            msg_value: info.msg_value,
        })
    }
);

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use alloy_primitives::{hex, B256};
    use brontes_classifier::test_utils::ClassifierTestUtils;
    use brontes_types::{
        constants::ETH_ADDRESS,
        db::token_info::{TokenInfo, TokenInfoWithAddress},
        normalized_actions::Action,
        TreeSearchBuilder,
    };

    use super::*;

    #[brontes_macros::test]
    async fn test_cowswap_settle() {
        let classifier_utils = ClassifierTestUtils::new().await;
        let swap =
            B256::from(hex!("23e459142f904e8aef751f1ca2b95bf75a45b1d7823692eb8b7eca3a9bf5c0fe"));

        let eq_action = Action::Batch(NormalizedBatch {
            protocol:            Protocol::Cowswap,
            trace_index:         0,
            solver:              Address::from_str("0x8646ee3c5e82b495be8f9fe2f2f213701eed0edc")
                .unwrap(),
            settlement_contract: Address::from_str("0x9008d19f58aabd9ed0d60971565aa8510560ab41")
                .unwrap(),
            user_swaps:          vec![NormalizedSwap {
                protocol:    Protocol::Cowswap,
                trace_index: 0,
                from:        Address::from_str("0x54e047e98c44b27f79dcfb6d2e35e41183b8dff6")
                    .unwrap(),
                recipient:   Address::from_str("0x54e047e98c44b27f79dcfb6d2e35e41183b8dff6")
                    .unwrap(),
                pool:        Address::from_str("0x9008d19f58aabd9ed0d60971565aa8510560ab41")
                    .unwrap(),
                token_in:    TokenInfoWithAddress {
                    address: Address::from_str("0xae78736cd615f374d3085123a210448e74fc6393")
                        .unwrap(),
                    inner:   TokenInfo { decimals: 18, symbol: "rETH".to_string() },
                },
                token_out:   TokenInfoWithAddress {
                    address: ETH_ADDRESS,
                    inner:   TokenInfo { decimals: 18, symbol: "ETH".to_string() },
                },
                amount_in:   U256::from_str("750005967291428997")
                    .unwrap()
                    .to_scaled_rational(18),
                amount_out:  U256::from_str("823443483581865908")
                    .unwrap()
                    .to_scaled_rational(18),
                msg_value:   U256::ZERO,
            }],
            solver_swaps:        Some(vec![]),
            msg_value:           U256::ZERO,
        });

        classifier_utils
            .contains_action(
                swap,
                0,
                eq_action,
                TreeSearchBuilder::default().with_action(Action::is_batch),
            )
            .await
            .unwrap();
    }
}
