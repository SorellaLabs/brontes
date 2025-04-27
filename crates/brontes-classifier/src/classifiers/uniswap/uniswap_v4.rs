use alloy_primitives::{Address, I256};
use brontes_macros::action_impl;
use brontes_pricing::Protocol;
use brontes_types::{
    db::token_info::TokenInfoWithAddress, normalized_actions::NormalizedSwap,
    structured_trace::CallInfo, ToScaledRational,
};
action_impl!(
    Protocol::UniswapV4,
    crate::UniswapV4::swapCall,
    Swap,
    [Swap],
    call_data: true,
    logs: true,
    return_data: true,
    |
    info: CallInfo,
    call_data: swapCall,
    return_data: swapReturn,
    log_data: UniswapV4SwapCallLogs,
    db_tx: &DB| {

        let pool_key = call_data.key;

        let (token_0, token_1) = (pool_key.currency0, pool_key.currency1);

        let zeroForOne = call_data.params.zeroForOne;


        let (t0_info, t1_info) = if token_0 == Address::default() {
            let t0_info = TokenInfoWithAddress::native_eth();
            let t1_info = db_tx.try_fetch_token_info(token_1)?;
            (t0_info, t1_info)
        } else {
            let t0_info = db_tx.try_fetch_token_info(token_0)?;
            let t1_info = db_tx.try_fetch_token_info(token_1)?;
            (t0_info, t1_info)
        };


        let logs = log_data.swap_field?;

        let recipient = logs.sender;

        let swapDelta = return_data.swapDelta;

        let (token_0_delta, token_1_delta) = split_balance_delta(swapDelta);


        let (amount_in, amount_out, token_in, token_out) = if zeroForOne {
            (
                token_0_delta.abs().to_scaled_rational(t0_info.decimals),
                token_1_delta.to_scaled_rational(t1_info.decimals),
                t0_info,
                t1_info,
            )
        } else {
            (
                token_1_delta.abs().to_scaled_rational(t1_info.decimals),
                token_0_delta.to_scaled_rational(t0_info.decimals),
                t1_info,
                t0_info,
            )
        };

        Ok(NormalizedSwap {
            protocol: Protocol::UniswapV4,
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

fn split_balance_delta(delta: I256) -> (i128, i128) {
    // 1) grab the big‐endian bytes (32 bytes)
    let be: [u8; 32] = delta.to_be_bytes();

    // 2) split into the high 16 bytes and low 16 bytes
    let high: [u8; 16] = be[0..16].try_into().unwrap();
    let low: [u8; 16] = be[16..32].try_into().unwrap();

    // 3) reinterpret each as signed i128
    let amount0 = i128::from_be_bytes(high);

    let amount1 = i128::from_be_bytes(low);

    (amount0, amount1)
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use alloy_primitives::{hex, Address, B256, U256};
    use brontes_classifier::test_utils::ClassifierTestUtils;
    use brontes_types::{
        db::token_info::TokenInfoWithAddress, normalized_actions::Action, Protocol::UniswapV4,
        TreeSearchBuilder,
    };

    use super::*;

    #[brontes_macros::test]
    async fn test_univ4_swap() {
        let classifier_utils = ClassifierTestUtils::new().await;

        classifier_utils.ensure_protocol(
            Protocol::UniswapV4,
            Address::new(hex!("000000000004444c5dc75cB358380D2e3dE08A90")),
            Address::default(),
            None,
            None,
            None,
            None,
            None,
        );

        let swap =
            B256::from(hex!("b9431a1a66d58bfb9c63daa566e766cfba38af66c9581708a447b410d418b01e"));

        let eq_action = Action::Swap(NormalizedSwap {
            protocol:    UniswapV4,
            trace_index: 76,
            from:        Address::new(hex!("66a9893cc07d91d95644aedd05d03f95e1dba8af")),
            recipient:   Address::new(hex!("66a9893cc07d91d95644aedd05d03f95e1dba8af")),
            pool:        Address::new(hex!("000000000004444c5dc75cB358380D2e3dE08A90")),
            token_in:    TokenInfoWithAddress::usdc(),
            amount_in:   U256::from_str("1371082002").unwrap().to_scaled_rational(6),
            token_out:   TokenInfoWithAddress::native_eth(),
            amount_out:  U256::from_str("824150165261952647")
                .unwrap()
                .to_scaled_rational(18),

            msg_value: U256::ZERO,
        });

        classifier_utils
            .contains_action(
                swap,
                4,
                eq_action,
                TreeSearchBuilder::default().with_action(Action::is_swap),
            )
            .await
            .unwrap();
    }
}
