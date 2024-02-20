// use alloy_primitives::{U256,Address};
use brontes_macros::action_impl;
use brontes_pricing::Protocol;
use brontes_types::{
    normalized_actions::NormalizedSwap,
    structured_trace::CallInfo, ToScaledRational,
};


use crate::ZeroXUniswapFeaure::sellToUniswapReturn;


action_impl!(
    Protocol::ZeroX,
    crate::ZeroXUniswapFeaure::sellToUniswapCall,
    Swap,
    [Swap],
    call_data: true,
    return_data: true,
    |
    info: CallInfo,
    call_data: sellToUniswapCall,
    return_data: sellToUniswapReturn,
    db_tx: &DB| {
        // if call_data.tokens.len() < 2 {
        //     return Err(:)
        // }
        let token_in = db_tx.try_fetch_token_info(call_data.tokens[0])?;
        let token_out = db_tx.try_fetch_token_info(call_data.tokens[call_data.tokens.len() - 1])?;
        let amount_in = call_data.sellAmount.to_scaled_rational(token_in.decimals);
        let amount_out = return_data.buyAmount.to_scaled_rational(token_out.decimals);

        // let ks = Report::;

        Ok(NormalizedSwap {
            protocol: Protocol::ZeroX,            
            trace_index: info.trace_idx,
            from: info.from_address,
            recipient: info.msg_sender,
            pool: info.target_address,
            token_in,
            token_out,
            amount_in,
            amount_out,
            msg_value: info.msg_value
        })
    }
);

