use alloy_primitives::U256;
use brontes_macros::action_impl;
use brontes_pricing::Protocol;
use brontes_types::{
    normalized_actions::{NormalizedBurn, NormalizedCollect, NormalizedMint, NormalizedSwap},
    structured_trace::CallInfo,
    ToScaledRational,
};

use crate::UniswapV3::{burnReturn, collectReturn, mintReturn, swapReturn};

action_impl!(
    Protocol::UniswapV3,
    crate::UniswapV3::swapCall,
    Swap,
    [Swap],
    call_data: true,
    return_data: true,
    |
    info: CallInfo,
    call_data: swapCall,
    return_data: swapReturn,
    db_tx: &DB| {
        let token_0_delta = return_data.amount0;
        let token_1_delta = return_data.amount1;
        let recipient = call_data.recipient;
        let details = db_tx.get_protocol_details_sorted(info.target_address)?;
        let [token_0, token_1] = [details.token0, details.token1];

        let t0_info = db_tx.try_fetch_token_info(token_0)?;
        let t1_info = db_tx.try_fetch_token_info(token_1)?;

        let (amount_in, amount_out, token_in, token_out) = if token_0_delta.is_negative() {
            (
                token_1_delta.to_scaled_rational(t1_info.decimals),
                token_0_delta.abs().to_scaled_rational(t0_info.decimals),
                t1_info,
                t0_info,
            )
        } else {
            (
                token_0_delta.to_scaled_rational(t0_info.decimals),
                token_1_delta.abs().to_scaled_rational(t1_info.decimals),
                t0_info,
                t1_info,
            )
        };

        Ok(NormalizedSwap {
            protocol: Protocol::UniswapV3,
            trace_index: info.trace_idx,
            from: info.from_address,
            pool: info.target_address,
            recipient,
            token_in,
            token_out,
            amount_in,
            amount_out,
            msg_value: info.msg_value
        })
    }
);
action_impl!(
    Protocol::UniswapV3,
    crate::UniswapV3::mintCall,
    Mint,
    [Mint],
    return_data: true,
    logs: true,
    call_data: true,
     |
     info: CallInfo,
     call_data: mintCall,
     return_data: mintReturn, _logs: UniswapV3MintCallLogs,  db_tx: &DB| {
         // needs extra logic based off of it uses the v3 position manager or not.
         let from_address = if info.from_address == alloy_primitives::hex!("C36442b4a4522E871399CD717aBDD847Ab11FE88") {
             call_data.recipient
         } else {
             info.from_address
         };



        let token_0_delta = return_data.amount0;
        let token_1_delta = return_data.amount1;
        let details = db_tx.get_protocol_details_sorted(info.target_address)?;
        let [token_0, token_1] = [details.token0, details.token1];

        let t0_info = db_tx.try_fetch_token_info(token_0)?;
        let t1_info = db_tx.try_fetch_token_info(token_1)?;

        let am0 = token_0_delta.to_scaled_rational(t0_info.decimals);
        let am1 = token_1_delta.to_scaled_rational(t1_info.decimals);

        Ok(NormalizedMint {
            protocol: Protocol::UniswapV3,
            trace_index: info.trace_idx,
            from: from_address,
            recipient: info.target_address,
            pool: info.target_address,
            token: vec![t0_info, t1_info],
            amount: vec![am0, am1],
        })
    }
);
action_impl!(
    Protocol::UniswapV3,
    crate::UniswapV3::burnCall,
    Burn,
    [Burn],
    return_data: true,
    |
    info: CallInfo,
    return_data: burnReturn,
    db_tx: &DB| {
        let token_0_delta: U256 = return_data.amount0;
        let token_1_delta: U256 = return_data.amount1;
        let details = db_tx.get_protocol_details_sorted(info.target_address)?;
        let [token_0, token_1] = [details.token0, details.token1];

        let t0_info = db_tx.try_fetch_token_info(token_0)?;
        let t1_info = db_tx.try_fetch_token_info(token_1)?;

        let am0 = token_0_delta.to_scaled_rational(t0_info.decimals);
        let am1 = token_1_delta.to_scaled_rational(t1_info.decimals);

        Ok(NormalizedBurn {
            protocol: Protocol::UniswapV3,
            recipient: info.from_address,
            pool: info.target_address,
            trace_index: info.trace_idx,
            from: info.from_address,
            token: vec![t0_info, t1_info],
            amount: vec![am0, am1],
        })
    }
);
action_impl!(
    Protocol::UniswapV3,
    crate::UniswapV3::collectCall,
    Collect,
    [Collect],
    call_data: true,
    return_data: true,
    |
    info: CallInfo,
    call_data: collectCall,
    return_data: collectReturn,
    db_tx: &DB
    | {
        let details = db_tx.get_protocol_details_sorted(info.target_address)?;
        let [token_0, token_1] = [details.token0, details.token1];

        let t0_info = db_tx.try_fetch_token_info(token_0)?;
        let t1_info = db_tx.try_fetch_token_info(token_1)?;

        let am0 = return_data.amount0.to_scaled_rational(t0_info.decimals);
        let am1 = return_data.amount1.to_scaled_rational(t1_info.decimals);

        Ok(NormalizedCollect {
            protocol: Protocol::UniswapV3,
            trace_index: info.trace_idx,
            from: info.from_address,
            recipient: call_data.recipient,
            pool: info.target_address,
            token: vec![t0_info, t1_info],
            amount: vec![am0, am1],
        })
    }
);

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use alloy_primitives::{hex, Address, B256};
    use brontes_classifier::test_utils::ClassifierTestUtils;
    use brontes_types::{
        db::token_info::TokenInfoWithAddress, normalized_actions::Action, Protocol::UniswapV3,
        TreeSearchBuilder,
    };

    use super::*;

    #[brontes_macros::test]
    async fn test_univ3_swap() {
        let classifier_utils = ClassifierTestUtils::new().await;
        let swap =
            B256::from(hex!("057f1d5b3ddabec1b8d78ac7181f562f755669494514f94a767247af800339b1"));

        let eq_action = Action::Swap(NormalizedSwap {
            protocol:    UniswapV3,
            trace_index: 2,
            from:        Address::new(hex!("A69babEF1cA67A37Ffaf7a485DfFF3382056e78C")),
            recipient:   Address::new(hex!("A69babEF1cA67A37Ffaf7a485DfFF3382056e78C")),
            pool:        Address::new(hex!("88e6A0c2dDD26FEEb64F039a2c41296FcB3f5640")),
            token_in:    TokenInfoWithAddress::weth(),
            amount_in:   U256::from_str("39283347298163243343")
                .unwrap()
                .to_scaled_rational(18),
            token_out:   TokenInfoWithAddress::usdc(),
            amount_out:  U256::from_str("98019119714").unwrap().to_scaled_rational(6),

            msg_value: U256::ZERO,
        });

        classifier_utils
            .contains_action(
                swap,
                0,
                eq_action,
                TreeSearchBuilder::default().with_action(Action::is_swap),
            )
            .await
            .unwrap();
    }

    #[brontes_macros::test]
    async fn test_uniswap_v3_mints() {
        let classifier_utils = ClassifierTestUtils::new().await;
        let mint =
            B256::from(hex!("0089210683170b3f17201c8abeafdc4c022a26c7af1e44d351556eaa48d0fee8"));

        let eq_action = Action::Mint(NormalizedMint {
            protocol:    UniswapV3,
            trace_index: 21,
            from:        Address::new(hex!("6b75d8AF000000e20B7a7DDf000Ba900b4009A80")),
            recipient:   Address::new(hex!("6b75d8AF000000e20B7a7DDf000Ba900b4009A80")),
            pool:        Address::new(hex!("3416cF6C708Da44DB2624D63ea0AAef7113527C6")),
            token:       vec![TokenInfoWithAddress::usdc(), TokenInfoWithAddress::usdt()],
            amount:      vec![
                U256::from_str("102642322850")
                    .unwrap()
                    .to_scaled_rational(6),
                U256::from_str("250137480130")
                    .unwrap()
                    .to_scaled_rational(6),
            ],
        });

        classifier_utils
            .contains_action(
                mint,
                0,
                eq_action,
                TreeSearchBuilder::default().with_action(Action::is_mint),
            )
            .await
            .unwrap();
    }

    #[brontes_macros::test]
    async fn test_uniswap_v3_burn() {
        let classifier_utils = ClassifierTestUtils::new().await;
        let burn =
            B256::from(hex!("f179f349434a59d0dc899fc03a5754c7e50f52de1709d9523e7cbd09c4ba13eb"));

        let eq_action = Action::Burn(NormalizedBurn {
            protocol:    UniswapV3,
            trace_index: 12,
            from:        Address::new(hex!("6b75d8AF000000e20B7a7DDf000Ba900b4009A80")),
            recipient:   Address::new(hex!("6b75d8AF000000e20B7a7DDf000Ba900b4009A80")),
            pool:        Address::new(hex!("3416cF6C708Da44DB2624D63ea0AAef7113527C6")),
            token:       vec![TokenInfoWithAddress::usdc(), TokenInfoWithAddress::usdt()],
            amount:      vec![
                U256::from_str("347057356182")
                    .unwrap()
                    .to_scaled_rational(6),
                U256::from_str("5793599811").unwrap().to_scaled_rational(6),
            ],
        });

        classifier_utils
            .contains_action(
                burn,
                0,
                eq_action,
                TreeSearchBuilder::default().with_action(Action::is_burn),
            )
            .await
            .unwrap();
    }

    #[brontes_macros::test]
    async fn test_uniswap_v3_collect() {
        let classifier_utils = ClassifierTestUtils::new().await;
        let collect =
            B256::from(hex!("f179f349434a59d0dc899fc03a5754c7e50f52de1709d9523e7cbd09c4ba13eb"));

        let eq_action = Action::Collect(NormalizedCollect {
            protocol:    UniswapV3,
            trace_index: 13,
            from:        Address::new(hex!("6b75d8AF000000e20B7a7DDf000Ba900b4009A80")),
            recipient:   Address::new(hex!("6b75d8AF000000e20B7a7DDf000Ba900b4009A80")),
            pool:        Address::new(hex!("3416cF6C708Da44DB2624D63ea0AAef7113527C6")),
            token:       vec![TokenInfoWithAddress::usdc(), TokenInfoWithAddress::usdt()],
            amount:      vec![
                U256::from_str("347081800129")
                    .unwrap()
                    .to_scaled_rational(6),
                U256::from_str("5793599811").unwrap().to_scaled_rational(6),
            ],
        });

        classifier_utils
            .contains_action(
                collect,
                0,
                eq_action,
                TreeSearchBuilder::default().with_action(Action::is_collect),
            )
            .await
            .unwrap();
    }
}
