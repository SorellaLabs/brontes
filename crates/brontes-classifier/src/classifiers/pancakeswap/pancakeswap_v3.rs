use alloy_primitives::U256;
use brontes_macros::action_impl;
use brontes_types::{
    normalized_actions::{NormalizedBurn, NormalizedCollect, NormalizedMint, NormalizedSwap},
    structured_trace::CallInfo,
    Protocol, ToScaledRational,
};

use crate::PancakeSwapV3::{burnReturn, collectReturn, mintReturn, swapReturn};

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
        let details = db_tx.get_protocol_details(info.target_address)?;
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
        let details = db_tx.get_protocol_details(info.target_address)?;
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
        let details = db_tx.get_protocol_details(info.target_address)?;
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
        let details = db_tx.get_protocol_details(info.target_address)?;
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

    use alloy_primitives::{hex, Address, B256, U256};
    use brontes_classifier::test_utils::ClassifierTestUtils;
    use brontes_types::{
        db::token_info::TokenInfoWithAddress, normalized_actions::Actions, Node,
        Protocol::PancakeSwapV3, ToScaledRational, TreeSearchArgs,
    };

    use super::*;

    #[brontes_macros::test]
    async fn test_pancake_v3_swap() {
        let classifier_utils = ClassifierTestUtils::new().await;
        let swap =
            B256::from(hex!("4a6cd8a23c0c832ccd645269a1a26b90f998b8f7837330fc38c92e090ec745f2"));

        let eq_action = Actions::Swap(NormalizedSwap {
            protocol:    PancakeSwapV3,
            trace_index: 115,
            from:        Address::new(hex!(
                "
                f081470f5C6FBCCF48cC4e5B82Dd926409DcdD67"
            )),
            recipient:   Address::new(hex!(
                "
                f081470f5C6FBCCF48cC4e5B82Dd926409DcdD67"
            )),
            pool:        Address::new(hex!(
                "2E8135bE71230c6B1B4045696d41C09Db0414226
                "
            )),
            token_in:    TokenInfoWithAddress::weth(),
            amount_in:   U256::from_str("212242932691433838")
                .unwrap()
                .to_scaled_rational(18),
            token_out:   TokenInfoWithAddress::usdc(),
            amount_out:  U256::from_str("529489490").unwrap().to_scaled_rational(6),

            msg_value: U256::ZERO,
        });

        let search_fn = |node: &Node<Actions>| TreeSearchArgs {
            collect_current_node:  node.data.is_swap(),
            child_node_to_collect: node
                .get_all_sub_actions()
                .iter()
                .any(|action| action.is_swap()),
        };

        classifier_utils
            .contains_action(swap, 1, eq_action, search_fn)
            .await
            .unwrap();
    }
}
