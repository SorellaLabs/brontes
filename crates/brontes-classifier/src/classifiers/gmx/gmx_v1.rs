use alloy_primitives::U256;
use brontes_macros::action_impl;
use brontes_pricing::Protocol;
use brontes_types::{
    normalized_actions::{NormalizedBurn, NormalizedCollect, NormalizedMint, NormalizedSwap},
    structured_trace::CallInfo,
    ToScaledRational,
};

action_impl!(
    Protocol::GMXV1,
    crate::GMXV1::swapCall,
    Swap,
    [Swap],
    call_data: true,
    logs: true,
    |
    info: CallInfo,
    call_data: swapCall,
    logs: GMXV1SwapCallLogs,
    db_tx: &DB| {

        let recipient=call_data._receiver;
        let log_data=logs.swap_field?;
        let token_in=log_data.tokenIn;
        let token_out=log_data.tokenOut;
        let amount_in=log_data.amountIn;
        let amount_out=log_data.amountOut;

        let token_in=db_tx.try_fetch_token_info(token_in)?;
        let token_out=db_tx.try_fetch_token_info(token_out)?;

        let amount_in=amount_in.to_scaled_rational(token_in.decimals);
        let amount_out=amount_out.to_scaled_rational(token_out.decimals);

        Ok(NormalizedSwap {
            protocol: Protocol::GMXV1,
            trace_index: info.trace_idx,
            from: info.from_address,
            recipient,
            pool: info.target_address,
            token_in,
            token_out,
            amount_in,
            amount_out,
            msg_value: info.msg_value
        })
    }
);

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use alloy_primitives::{hex, Address, B256};
    use brontes_classifier::test_utils::ClassifierTestUtils;
    use brontes_types::{
        db::token_info::TokenInfoWithAddress, normalized_actions::Action, Protocol::GMXV1,
        TreeSearchBuilder,
    };

    use super::*;

    #[brontes_macros::test]
    async fn test_univ3_swap() {
        let classifier_utils = ClassifierTestUtils::new().await;
        let swap =
            B256::from(hex!("057f1d5b3ddabec1b8d78ac7181f562f755669494514f94a767247af800339b1"));

        let eq_action = Action::Swap(NormalizedSwap {
            protocol:    GMXV1,
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
            protocol:    GMXV1,
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
            protocol:    GMXV1,
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
            protocol:    GMXV1,
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
