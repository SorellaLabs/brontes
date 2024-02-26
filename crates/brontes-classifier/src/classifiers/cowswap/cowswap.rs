use alloy_primitives::{Address, U256};
use brontes_database::libmdbx::{DBWriter, LibmdbxReader};
use brontes_macros::action_impl;
use brontes_pricing::Protocol;
use brontes_types::{
    normalized_actions::{NormalizedBatch, NormalizedSwap},
    structured_trace::CallInfo,
    ToScaledRational,
};
use eyre::Error;
use Protocol::Cowswap;

use crate::CowswapGPv2Settlement::Trade;

fn create_normalized_swap<DB: LibmdbxReader + DBWriter>(
    trade: &Trade,
    db_tx: &DB,
    protocol: Protocol,
    pool_address: Address,
    trace_index: u64,
) -> Result<NormalizedSwap, Error> {
    let token_in_info = db_tx.try_fetch_token_info(trade.sellToken)?;
    let token_out_info = db_tx.try_fetch_token_info(trade.buyToken)?;

    let amount_in = trade.sellAmount.to_scaled_rational(token_in_info.decimals);
    let amount_out = trade.buyAmount.to_scaled_rational(token_out_info.decimals);

    Ok(NormalizedSwap {
        protocol,
        trace_index,
        from: trade.owner,
        recipient: trade.owner,
        pool: pool_address,
        token_in: token_in_info,
        token_out: token_out_info,
        amount_in,
        amount_out,
        msg_value: U256::ZERO,
    })
}

action_impl!(
    Protocol::Cowswap,
    crate::CowswapGPv2Settlement::settleCall,
    Batch,
    [Trade*],
    call_data: true,
    logs: true,
    |info: CallInfo, _call_data: settleCall, log_data: CowswapsettleCallLogs, db_tx: &DB| {
        let user_swaps: Vec<NormalizedSwap> = log_data.Trade_field.iter().map(
            |trade| {
                create_normalized_swap(trade, db_tx, Cowswap, info.target_address, 0)
            }
        ).collect::<Result<Vec<NormalizedSwap>, Error>>()?;

        Ok(NormalizedBatch {
            protocol: Cowswap,
            trace_index: info.trace_idx,
            solver: info.msg_sender,
            settlement_contract: info.target_address,
            user_swaps,
            solver_swaps: None,
            msg_value: info.msg_value,
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
    |info: CallInfo, _call_data: swapCall, log_data: CowswapswapCallLogs, db_tx: &DB| {
        let tx_to = info.target_address;
        let swap = create_normalized_swap(&log_data.Trade_field, db_tx, Cowswap, tx_to, 0)?;

        Ok(NormalizedBatch {
            protocol: Cowswap,
            trace_index: info.trace_idx,
            solver: info.msg_sender,
            settlement_contract: tx_to,
            user_swaps: vec![swap],
            solver_swaps: None,
            msg_value: info.msg_value,
        })
    }
);
