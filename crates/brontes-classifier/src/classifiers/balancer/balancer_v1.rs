use brontes_macros::action_impl;
use brontes_pricing::Protocol;
use brontes_types::{
    normalized_actions::NormalizedSwap, structured_trace::CallInfo, ToScaledRational,
};

use crate::{
    BalancerV1::{swapExactAmountInReturn, swapExactAmountOutReturn},
    NormalizedNewPool,
};

action_impl!(
    Protocol::BalancerV1,
    crate::BalancerV1::swapExactAmountInCall,
    Swap,
    [Swap],
    call_data: true,
    return_data: true,
    |
    info: CallInfo,
    call_data: swapExactAmountInCall,
    return_data: swapExactAmountInReturn,
    db_tx: &DB| {
        let token_in = db_tx.try_fetch_token_info(call_data.tokenIn)?;
        let token_out = db_tx.try_fetch_token_info(call_data.tokenOut)?;
        let amount_in = call_data.tokenAmountIn.to_scaled_rational(token_in.decimals);
        let amount_out = return_data.tokenAmountOut.to_scaled_rational(token_out.decimals);

        Ok(NormalizedSwap {
            protocol: Protocol::BalancerV1,
            trace_index: info.trace_idx,
            from: info.from_address,
            recipient: info.msg_sender,
            pool: info.target_address,
            token_in,
            token_out,
            amount_in,
            amount_out,
            msg_value: info.msg_value,
        })
    }
);

action_impl!(
    Protocol::BalancerV1,
    crate::BalancerV1::swapExactAmountOutCall,
    Swap,
    [Swap],
    call_data: true,
    return_data: true,
    |
    info: CallInfo,
    call_data: swapExactAmountOutCall,
    return_data: swapExactAmountOutReturn,
    db_tx: &DB| {
        let token_in = db_tx.try_fetch_token_info(call_data.tokenIn)?;
        let token_out = db_tx.try_fetch_token_info(call_data.tokenOut)?;
        let amount_in = return_data.tokenAmountIn.to_scaled_rational(token_in.decimals);
        let amount_out = call_data.tokenAmountOut.to_scaled_rational(token_out.decimals);

        Ok(NormalizedSwap {
            protocol: Protocol::BalancerV1,
            trace_index: info.trace_idx,
            from: info.from_address,
            recipient: info.msg_sender,
            pool: info.target_address,
            token_in,
            token_out,
            amount_in,
            amount_out,
            msg_value: info.msg_value,
        })
    }
);

action_impl!(
    Protocol::BalancerV1,
    crate::BalancerV1::bindCall,
    NewPool,
    [],
    call_data: true,
    |
    info: CallInfo,
    call_data: bindCall,
    db_tx: &DB| {

        let pool = db_tx.get_protocol_details(info.target_address).unwrap_or_default();

        let mut tokens = pool.get_tokens();
        tokens.push(call_data.token);

        Ok( NormalizedNewPool {
            protocol: Protocol::BalancerV1,
            trace_index: info.trace_idx,
            pool_address: info.target_address,
            tokens
        })
    }
);

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use alloy_primitives::{hex, Address, B256, U256};
    use brontes_classifier::test_utils::ClassifierTestUtils;
    use brontes_types::{
        db::token_info::{TokenInfo, TokenInfoWithAddress},
        normalized_actions::Action,
        Protocol::BalancerV1,
        TreeSearchBuilder,
    };

    use super::*;

    #[brontes_macros::test]
    async fn test_balancer_v1_swap() {
        let classifier_utils = ClassifierTestUtils::new().await;
        let swap =
            B256::from(hex!("c832c2dcdbb2e3ca021ccb594ded9bf3308f2b4b5a90f615aa8e053c0e180a35"));

        let eq_action = Action::Swap(NormalizedSwap {
            protocol:    BalancerV1,
            trace_index: 11,
            from:        Address::new(hex!("0eae044f00B0aF300500F090eA00027097d03000")),
            recipient:   Address::new(hex!("0eae044f00B0aF300500F090eA00027097d03000")),
            pool:        Address::new(hex!("92E7Eb99a38C8eB655B15467774C6d56Fb810BC9")),
            token_in:    TokenInfoWithAddress::usdc(),
            amount_in:   U256::from_str("72712976").unwrap().to_scaled_rational(6),
            token_out:   TokenInfoWithAddress {
                address: Address::new(hex!("f8C3527CC04340b208C854E985240c02F7B7793f")),
                inner:   TokenInfo { decimals: 18, symbol: "FRONT".to_string() },
            },
            amount_out:  U256::from_str("229136254468181839981")
                .unwrap()
                .to_scaled_rational(18),

            msg_value: U256::ZERO,
        });

        classifier_utils
            .contains_action(
                swap,
                1,
                eq_action,
                TreeSearchBuilder::default().with_action(Action::is_swap),
            )
            .await
            .unwrap();
    }

    #[brontes_macros::test]
    async fn test_balancer_v1_bind() {
        let classifier_utils = ClassifierTestUtils::new().await;

        classifier_utils.ensure_protocol(
            Protocol::BalancerV1,
            Address::from(hex!("9A2181cf0bC57FC0177517dB21d457BDd1b2b32e")),
            Address::default(),
            None,
            None,
            None,
            None,
            None,
        );

        let classifier_utils = ClassifierTestUtils::new().await;
        let bind_tx_hash =
            B256::from(hex!("400146b33d7e9c5ba7626c927a93ddc4dbd5e13161609011681450c19eaebfed"));

        let expected_action = Action::NewPool(NormalizedNewPool {
            protocol:     Protocol::BalancerV1,
            trace_index:  0,
            pool_address: Address::new(hex!("9A2181cf0bC57FC0177517dB21d457BDd1b2b32e")),
            tokens:       vec![Address::new(hex!("5eD9e47679422c2F78568af8728EC3C3C8591146"))],
        });

        classifier_utils
            .contains_action(
                bind_tx_hash,
                0,
                expected_action,
                TreeSearchBuilder::default().with_action(Action::is_new_pool),
            )
            .await
            .unwrap();
    }
}
