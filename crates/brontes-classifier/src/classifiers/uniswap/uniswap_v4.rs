use alloy_sol_types::{sol_data, SolType};
use alloy_primitives::{ Address, U256};
use alloy_primitives::{keccak256};
use brontes_macros::action_impl;
use brontes_pricing::Protocol;
use brontes_types::{
    normalized_actions::NormalizedSwap,
    structured_trace::CallInfo,
    ToScaledRational,
};


action_impl!(
    Protocol::UniswapV4,
    crate::UniswapV4::swapCall,
    Swap,
    [Swap],
    call_data: true,
    return_data: true,
    |
    info: CallInfo,
    call_data: swapCall,
    return_data: swapReturn,
    db_tx: &DB| {

        type PoolKey = (sol_data::Address, sol_data::Address, sol_data::Uint<256>, sol_data::Uint<256>, sol_data::Address);

        let call_params=call_data.params;
        let pool_key=call_data.key;
        
        let encoded_data = PoolKey::abi_encode(&
            (
                pool_key.currency0,
                pool_key.currency1,
                U256::from(pool_key.fee),
                U256::from(pool_key.tickSpacing as u64),
                pool_key.hooks,
            )
        );
        let target_address = Address::from_slice(&keccak256(&encoded_data)[..20]);
        let exact_in=call_params.amountSpecified.is_negative();
        let zero_for_one=call_params.zeroForOne;
        
        let swap_delta=return_data.swapDelta;

        let t0_info = db_tx.try_fetch_token_info(pool_key.currency0)?;
        let t1_info = db_tx.try_fetch_token_info(pool_key.currency1)?;
        let (token_in, token_out)= if zero_for_one {
            (t0_info, t1_info)
        }else {
            (t1_info, t0_info)
        };
        let (amount_in, amount_out) =
            if exact_in {
                (call_params.amountSpecified.abs().to_scaled_rational(token_in.decimals),swap_delta.abs().to_scaled_rational(token_out.decimals))
            }else {
                (swap_delta.abs().to_scaled_rational(token_in.decimals),call_params.amountSpecified.abs().to_scaled_rational(token_out.decimals))
            };

        Ok(NormalizedSwap {
            protocol: Protocol::UniswapV4,
            trace_index: info.trace_idx,
            from: info.from_address,
            pool: target_address,
            recipient: info.msg_sender,
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
        db::token_info::TokenInfoWithAddress, normalized_actions::Action, Protocol::UniswapV4,
        TreeSearchBuilder,
    };

    use super::*;

    #[brontes_macros::test]
    async fn test_univ4_swap() {
        let classifier_utils = ClassifierTestUtils::new().await;
        let swap =
            B256::from(hex!("057f1d5b3ddabec1b8d78ac7181f562f755669494514f94a767247af800339b1"));

        let eq_action = Action::Swap(NormalizedSwap {
            protocol:    UniswapV4,
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
    async fn test_uniswap_v4_mints() {
        let classifier_utils = ClassifierTestUtils::new().await;
        let mint =
            B256::from(hex!("0089210683170b3f17201c8abeafdc4c022a26c7af1e44d351556eaa48d0fee8"));

        let eq_action = Action::Mint(NormalizedMint {
            protocol:    UniswapV4,
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
    async fn test_uniswap_v4_burn() {
        let classifier_utils = ClassifierTestUtils::new().await;
        let burn =
            B256::from(hex!("f179f349434a59d0dc899fc03a5754c7e50f52de1709d9523e7cbd09c4ba13eb"));

        let eq_action = Action::Burn(NormalizedBurn {
            protocol:    UniswapV4,
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
    async fn test_uniswap_v4_collect() {
        let classifier_utils = ClassifierTestUtils::new().await;
        let collect =
            B256::from(hex!("f179f349434a59d0dc899fc03a5754c7e50f52de1709d9523e7cbd09c4ba13eb"));

        let eq_action = Action::Collect(NormalizedCollect {
            protocol:    UniswapV4,
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
