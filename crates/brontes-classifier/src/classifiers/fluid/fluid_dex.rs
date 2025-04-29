use brontes_macros::action_impl;
use brontes_pricing::Protocol;
use brontes_types::{
    normalized_actions::NormalizedSwap, structured_trace::CallInfo, ToScaledRational,
};

action_impl!(
    Protocol::FluidDex,
    crate::FluidDex::swapInCall,
    Swap,
    [Swap],
    call_data: false,
    logs: true,
    |info: CallInfo,
     log_data: FluidDexSwapInCallLogs,
     db_tx: &DB| {

        let ev = log_data.swap_field?;

        // fetch token0/token1 and their metadata
        let details = db_tx.get_protocol_details_sorted(info.target_address)?;
        let [token0, token1] = [details.token0, details.token1];
        let t0_info = db_tx.try_fetch_token_info(token0)?;
        let t1_info = db_tx.try_fetch_token_info(token1)?;

        // pick in/out based on direction
        let (token_in, token_out, raw_in, raw_out) = if ev.swap0to1 {
            (t0_info, t1_info, ev.amountIn, ev.amountOut)
        } else {
            (t1_info, t0_info, ev.amountIn, ev.amountOut)
        };

        // scale to human‐readable Rational
        let amount_in  = raw_in.to_scaled_rational(token_in.decimals);
        let amount_out = raw_out.to_scaled_rational(token_out.decimals);
        let recipient  = info.from_address;

        Ok(NormalizedSwap {
            protocol:    Protocol::FluidDex,
            pool:        info.target_address,
            trace_index: info.trace_idx,
            from:        info.from_address,
            recipient,
            token_in,
            token_out,
            amount_in,
            amount_out,
            msg_value:   info.msg_value,
        })
    }
);

action_impl!(
    Protocol::FluidDex,
    crate::FluidDex::swapInWithCallbackCall,
    Swap,
    [Swap],
    call_data: false,
    logs: true,
    |info: CallInfo,
     log_data: FluidDexSwapInWithCallbackCallLogs,
     db_tx: &DB| {
        let ev = log_data.swap_field?;
        let details = db_tx.get_protocol_details_sorted(info.target_address)?;
        let [token0, token1] = [details.token0, details.token1];
        let t0_info = db_tx.try_fetch_token_info(token0)?;
        let t1_info = db_tx.try_fetch_token_info(token1)?;

        let (token_in, token_out, raw_in, raw_out) = if ev.swap0to1 {
            (t0_info.clone(), t1_info.clone(), ev.amountIn, ev.amountOut)
        } else {
            (t1_info.clone(), t0_info.clone(), ev.amountIn, ev.amountOut)
        };

        let amount_in  = raw_in.to_scaled_rational(token_in.decimals);
        let amount_out = raw_out.to_scaled_rational(token_out.decimals);
        let recipient  = info.from_address;

        Ok(NormalizedSwap {
            protocol:    Protocol::FluidDex,
            pool:        info.target_address,
            trace_index: info.trace_idx,
            from:        info.from_address,
            recipient,
            token_in,
            token_out,
            amount_in,
            amount_out,
            msg_value:   info.msg_value,
        })
    }
);

action_impl!(
    Protocol::FluidDex,
    crate::FluidDex::swapOutCall,
    Swap,
    [Swap],
    logs: true,
    |info: CallInfo,
     log_data: FluidDexSwapOutCallLogs,
     db_tx: &DB| {
        let ev = log_data.swap_field?;
        let details = db_tx.get_protocol_details_sorted(info.target_address)?;
        let [token0, token1] = [details.token0, details.token1];
        let t0_info = db_tx.try_fetch_token_info(token0)?;
        let t1_info = db_tx.try_fetch_token_info(token1)?;

        let (token_in, token_out, raw_in, raw_out) = if ev.swap0to1 {
            (t0_info.clone(), t1_info.clone(), ev.amountIn, ev.amountOut)
        } else {
            (t1_info.clone(), t0_info.clone(), ev.amountIn, ev.amountOut)
        };

        let amount_in  = raw_in.to_scaled_rational(token_in.decimals);
        let amount_out = raw_out.to_scaled_rational(token_out.decimals);
        let recipient  = info.from_address;

        Ok(NormalizedSwap {
            protocol:    Protocol::FluidDex,
            pool:        info.target_address,
            trace_index: info.trace_idx,
            from:        info.from_address,
            recipient,
            token_in,
            token_out,
            amount_in,
            amount_out,
            msg_value:   info.msg_value,
        })
    }
);

action_impl!(
    Protocol::FluidDex,
    crate::FluidDex::swapOutWithCallbackCall,
    Swap,
    [Swap],
    logs: true,
    |info: CallInfo,
     log_data: FluidDexSwapOutWithCallbackCallLogs,
     db_tx: &DB| {
        let ev = log_data.swap_field?;
        let details = db_tx.get_protocol_details_sorted(info.target_address)?;
        let [token0, token1] = [details.token0, details.token1];
        let t0_info = db_tx.try_fetch_token_info(token0)?;
        let t1_info = db_tx.try_fetch_token_info(token1)?;

        let (token_in, token_out, raw_in, raw_out) = if ev.swap0to1 {
            (t0_info.clone(), t1_info.clone(), ev.amountIn, ev.amountOut)
        } else {
            (t1_info.clone(), t0_info.clone(), ev.amountIn, ev.amountOut)
        };

        let amount_in  = raw_in.to_scaled_rational(token_in.decimals);
        let amount_out = raw_out.to_scaled_rational(token_out.decimals);
        let recipient  = info.from_address;

        Ok(NormalizedSwap {
            protocol:    Protocol::FluidDex,
            pool:        info.target_address,
            trace_index: info.trace_idx,
            from:        info.from_address,
            recipient,
            token_in,
            token_out,
            amount_in,
            amount_out,
            msg_value:   info.msg_value,
        })
    }
);

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use alloy_primitives::{hex, Address, B256, U256};
    use brontes_classifier::test_utils::ClassifierTestUtils;
    use brontes_types::{
        db::token_info::TokenInfoWithAddress,
        normalized_actions::{Action, NormalizedSwap},
        Protocol::FluidDex,
        TreeSearchBuilder,
    };

    use super::*;

    #[brontes_macros::test]
    async fn test_fluiddex_swap() {
        let classifier_utils = ClassifierTestUtils::new().await;

        // the tx hash to replay
        let swap =
            B256::from(hex!("813062b3c13a3357327f018c44d9b809be708938244eb99bda6872b053c4ce84"));

        // expected NormalizedSwap action
        let eq_action = Action::Swap(NormalizedSwap {
            protocol:    FluidDex,
            trace_index: 2,
            from:        Address::new(hex!("A69babEF1cA67A37Ffaf7a485DfFF3382056e78C")),
            recipient:   Address::new(hex!("A69babEF1cA67A37Ffaf7a485DfFF3382056e78C")),
            pool:        Address::new(hex!("836951EB21F3Df98273517B7249dCEFF270d34bf")),
            token_in:    TokenInfoWithAddress::usdc(),
            amount_in:   U256::from_str("2998490001568")
                .unwrap()
                .to_scaled_rational(6),
            token_out:   TokenInfoWithAddress::weth(),
            amount_out:  U256::from_str("1926403430556070000000")
                .unwrap()
                .to_scaled_rational(18),
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
