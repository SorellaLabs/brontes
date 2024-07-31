use alloy_primitives::U256;
use brontes_macros::action_impl;
use brontes_types::{
    normalized_actions::{NormalizedBurn, NormalizedCollect, NormalizedMint, NormalizedSwap},
    structured_trace::CallInfo,
    Protocol, ToScaledRational,
};

action_impl!(
    Protocol::PancakeSwapV3,
    crate::PancakeSwapV3::swapCall,
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
            protocol: Protocol::PancakeSwapV3,
            trace_index: info.trace_idx,
            from: info.from_address,
            recipient,
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
    Protocol::PancakeSwapV3,
    crate::PancakeSwapV3::mintCall,
    Mint,
    [Mint],
    return_data: true,
    call_data: true,
    |
    info: CallInfo,
    call_data: mintCall,
     return_data: mintReturn,  db_tx: &DB| {
        let token_0_delta = return_data.amount0;
        let token_1_delta = return_data.amount1;
        let details = db_tx.get_protocol_details_sorted(info.target_address)?;
        let [token_0, token_1] = [details.token0, details.token1];

        let t0_info = db_tx.try_fetch_token_info(token_0)?;
        let t1_info = db_tx.try_fetch_token_info(token_1)?;

        let am0 = token_0_delta.to_scaled_rational(t0_info.decimals);
        let am1 = token_1_delta.to_scaled_rational(t1_info.decimals);

        Ok(NormalizedMint {
            protocol: Protocol::PancakeSwapV3,
            trace_index: info.trace_idx,
            from: info.from_address,
            recipient: call_data.recipient,
            pool: info.target_address,
            token: vec![t0_info, t1_info],
            amount: vec![am0, am1],
        })
    }
);
action_impl!(
    Protocol::PancakeSwapV3,
    crate::PancakeSwapV3::burnCall,
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
            protocol: Protocol::PancakeSwapV3,
            trace_index: info.trace_idx,
            from: info.from_address,
            recipient: info.target_address,
            pool: info.target_address,
            token: vec![t0_info, t1_info],
            amount: vec![am0, am1],
        })
    }
);
action_impl!(
    Protocol::PancakeSwapV3,
    crate::PancakeSwapV3::collectCall,
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
            protocol: Protocol::PancakeSwapV3,
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
        db::token_info::{TokenInfo, TokenInfoWithAddress},
        normalized_actions::Action,
        Protocol::PancakeSwapV3,
        TreeSearchBuilder,
    };

    use super::*;

    #[brontes_macros::test]
    async fn test_pancake_v3_swap() {
        let classifier_utils = ClassifierTestUtils::new().await;
        classifier_utils.ensure_protocol(
            Protocol::PancakeSwapV3,
            Address::new(hex!("Ed4D5317823Ff7BC8BB868C1612Bb270a8311179")),
            Address::new(hex!("186eF81fd8E77EEC8BfFC3039e7eC41D5FC0b457")),
            Some(TokenInfoWithAddress::usdt().address),
            None,
            None,
            None,
            None,
        );
        let token_info = TokenInfoWithAddress {
            address: Address::new(hex!("186eF81fd8E77EEC8BfFC3039e7eC41D5FC0b457")),
            inner:   TokenInfo { decimals: 18, symbol: "INSP".to_owned() },
        };

        classifier_utils.ensure_token(TokenInfoWithAddress::usdt());
        classifier_utils.ensure_token(token_info.clone());

        let swap =
            B256::from(hex!("649b792d819826302eb2859a9a1b8f3bb1a78bb5c480d433cdc6cc4ab129337f"));

        let eq_action = Action::Swap(NormalizedSwap {
            protocol:    PancakeSwapV3,
            trace_index: 1,
            from:        Address::new(hex!("1b81D678ffb9C0263b24A97847620C99d213eB14")),
            recipient:   Address::new(hex!("6Dbe61E7c69AF3bF5d20C15494bD69eD1905A335")),
            pool:        Address::new(hex!("Ed4D5317823Ff7BC8BB868C1612Bb270a8311179")),
            token_in:    token_info,
            amount_in:   U256::from_str("8888693999999999016960")
                .unwrap()
                .to_scaled_rational(18),
            token_out:   TokenInfoWithAddress::usdt(),
            amount_out:  U256::from_str("1568955344").unwrap().to_scaled_rational(6),
            msg_value:   U256::ZERO,
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
}
