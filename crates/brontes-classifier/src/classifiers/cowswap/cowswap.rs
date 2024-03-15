use alloy_primitives::{Address, U256};
use brontes_database::libmdbx::{DBWriter, LibmdbxReader};
use brontes_macros::action_impl;
use brontes_pricing::Protocol;
use brontes_types::{
    normalized_actions::{NormalizedAggregator, NormalizedBatch, NormalizedSwap},
    structured_trace::CallInfo,
    ToScaledRational,
};
use eyre::Error;
use Protocol::Cowswap;

use crate::CowswapGPv2Settlement::Trade;

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

fn create_normalized_swap_i(
    protocol: Protocol,
    trace_index: u64,
) -> Result<NormalizedAggregator, Error> {
    let aggregate = NormalizedAggregator {
        protocol,
        trace_index,
        from: Address::ZERO,
        recipient: Address::ZERO,
        msg_value: U256::ZERO,
        child_actions: vec![],
    };

    Ok(aggregate)
}

action_impl!(
    Protocol::Cowswap,
    crate::CowswapGPv2Settlement::swapCall,
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
    crate::CowswapGPv2Settlement::settleCall,
    Aggregator,
    [..Trade*],
    call_data: true,
    logs: true,
    |info: CallInfo, _call_data: settleCall, log_data: CowswapSettleCallLogs, db_tx: &DB| {


        Ok(create_normalized_swap_i(Cowswap, info.trace_idx)?)
    }
);

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use alloy_primitives::{hex, B256};
    use brontes_classifier::test_utils::ClassifierTestUtils;
    use brontes_types::{
        db::token_info::TokenInfo, normalized_actions::Actions,
        TreeSearchBuilder,
    };

    use super::*;

    #[brontes_macros::test]
    async fn test_cowswap_settle() {
        let classifier_utils = ClassifierTestUtils::new().await;
        let swap =
            B256::from(hex!("d3bc9024e9c65b2d807dbb956fc9e35a97d131f57d355f806e6cc60de16e18fd"));

        let eq_action = Actions::Aggregator(NormalizedAggregator {
            protocol: Protocol::Cowswap,
            trace_index: 0,
            from: Address::ZERO,
            recipient: Address::ZERO,
            msg_value: U256::ZERO,
            child_actions: vec![],
         });


        classifier_utils
            .contains_action(
                swap,
                0,
                eq_action,
                TreeSearchBuilder::default().with_action(Actions::is_aggregator),
            )
            .await
            .unwrap();
    }

}