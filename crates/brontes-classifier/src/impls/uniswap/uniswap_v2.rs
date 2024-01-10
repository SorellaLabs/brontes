use alloy_primitives::{Address, Bytes, U256};
use alloy_sol_types::{SolCall, SolEvent};
use brontes_database_libmdbx::{implementation::tx::LibmdbxTx, tables::AddressToTokens};
use brontes_macros::{action_dispatch, action_impl};
use brontes_pricing::types::PoolUpdate;
use brontes_types::normalized_actions::{Actions, NormalizedBurn, NormalizedMint, NormalizedSwap};
use reth_db::{mdbx::RO, transaction::DbTx};
use reth_rpc_types::Log;
use tokio::sync::mpsc::UnboundedSender;

use crate::{
    enum_unwrap, ActionCollection, IntoAction, StaticReturnBindings,
    UniswapV2::{burnCall, mintCall, swapCall, Burn, Mint, Swap, UniswapV2Calls},
};
action_impl!(
    V2SwapImpl,
    Swap,
    swapCall,
    [Swap],
    UniswapV2,
    call_data: true,
    logs: true,
    |trace_index, from_address: Address, target_address: Address, call_data: swapCall, data: Swap, db_tx: &LibmdbxTx<RO>| {
        let recipient = call_data.to;

        let tokens = db_tx.get::<AddressToTokens>(target_address).ok()??;
        let [token_0, token_1] = [tokens.token0, tokens.token1];

        let amount_0_in: U256 = data.amount0In;
        if amount_0_in == U256::ZERO {
            return Some(NormalizedSwap {
                pool: target_address,
                trace_index,
                from: from_address,
                recipient,
                token_in: token_1,
                token_out: token_0,
                amount_in: data.amount1In,
                amount_out: data.amount0Out,
            })
        } else {
            return Some(NormalizedSwap {
                trace_index,
                pool: target_address,
                from: from_address,
                recipient,
                token_in: token_0,
                token_out: token_1,
                amount_in: data.amount0In,
                amount_out: data.amount1Out,
            })
        }
    }
);

action_impl!(
    V2MintImpl,
    Mint,
    mintCall,
    [Mint],
    UniswapV2,
    logs: true,
    call_data: true,
    |trace_index,
     from_address: Address,
     target_address: Address,
     call_data: mintCall,
     log_data: Mint, db_tx: &LibmdbxTx<RO>| {
        let tokens = db_tx.get::<AddressToTokens>(target_address).ok()??;
        let [token_0, token_1] = [tokens.token0, tokens.token1];
        Some(NormalizedMint {
            recipient: call_data.to,
            from: from_address,
            trace_index,
            to: target_address,
            token: vec![token_0, token_1],
            amount: vec![log_data.amount0, log_data.amount1],
        })
    }
);
action_impl!(
    V2BurnImpl,
    Burn,
    burnCall,
    [Burn],
    UniswapV2,
    call_data: true,
    logs: true,
    |trace_index,
     from_address: Address,
     target_address: Address,
     call_data: burnCall,
     log_data: Burn, db_tx: &LibmdbxTx<RO>| {
        let tokens = db_tx.get::<AddressToTokens>(target_address).ok()??;
        let [token_0, token_1] = [tokens.token0, tokens.token1];
        Some(NormalizedBurn {
            recipient: call_data.to,
            to: target_address,
            trace_index,
            from: from_address,
            token: vec![token_0, token_1],
            amount: vec![log_data.amount0, log_data.amount1],
        })
    }
);

action_dispatch!(UniswapV2Classifier, V2SwapImpl, V2BurnImpl, V2MintImpl);
