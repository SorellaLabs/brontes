use alloy_primitives::U256;
use brontes_macros::action_impl;
use brontes_pricing::Protocol;
use brontes_types::{
    normalized_actions::{NormalizedBatch, NormalizedSwap},
    structured_trace::CallInfo, ToScaledRational,
};


action_impl!(
    Protocol::Cowswap,
    crate::CowswapGPv2Settlement::settleCall,
    Batch,
    [Trade*],
    call_data: true,
    logs: true,
    |
    info: CallInfo,
    _call_data: settleCall,
    log_data: CowswapsettleCallLogs,
    db_tx: &DB| {

        let user_swaps: Vec<NormalizedSwap> = log_data.Trade_field.into_iter().map(|trade| {
            let token_in_info = db_tx.try_fetch_token_info(trade.sellToken).unwrap();
            let token_out_info = db_tx.try_fetch_token_info(trade.buyToken).unwrap();

            let amount_in = trade.sellAmount.to_scaled_rational(token_in_info.decimals);
            let amount_out = trade.buyAmount.to_scaled_rational(token_out_info.decimals);
    
    
            NormalizedSwap { 
                protocol: Protocol::Cowswap, 
                trace_index: 0, 
                from: trade.owner, 
                recipient: trade.owner, 
                pool: info.target_address, 
                token_in: token_in_info, 
                token_out: token_out_info, 
                amount_in,
                amount_out,
                msg_value: U256::ZERO 
            }

        }).collect();


        Ok(NormalizedBatch{ 
            protocol: Protocol::Cowswap, 
            trace_index: info.trace_idx, 
            solver: info.msg_sender, 
            settlement_contract: info.target_address, 
            user_swaps, 
            solver_swaps: None, 
            msg_value: info.msg_value
        })
    }
);


action_impl!(
    Protocol::Cowswap,
    crate::CowswapGPv2Settlement::swapCall,
    Batch,
    [Trade],
    call_data: true,
    logs: true,
    |
    info: CallInfo,
    _call_data: swapCall,
    log_data: CowswapswapCallLogs,
    db_tx: &DB| {

        let swap = {
            let trade = log_data.Trade_field;

            let token_in_info = db_tx.try_fetch_token_info(trade.sellToken).unwrap();
            let token_out_info = db_tx.try_fetch_token_info(trade.buyToken).unwrap();

            let amount_in = trade.sellAmount.to_scaled_rational(token_in_info.decimals);
            let amount_out = trade.buyAmount.to_scaled_rational(token_out_info.decimals);
    
    
            NormalizedSwap { 
                protocol: Protocol::Cowswap, 
                trace_index: 0, 
                from: trade.owner, 
                recipient: trade.owner, 
                pool: info.target_address, 
                token_in: token_in_info, 
                token_out: token_out_info, 
                amount_in,
                amount_out,
                msg_value: U256::ZERO 
            }
        };

        Ok(NormalizedBatch{ 
            protocol: Protocol::Cowswap, 
            trace_index: info.trace_idx, 
            solver: info.msg_sender, 
            settlement_contract: info.target_address, 
            user_swaps: vec![swap], 
            solver_swaps: None, 
            msg_value: info.msg_value
        })
    }
);


