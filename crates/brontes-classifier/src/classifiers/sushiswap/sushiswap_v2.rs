use alloy_primitives::{Address, U256};
use brontes_database_libmdbx::{implementation::tx::LibmdbxTx, tables::AddressToTokens};
use brontes_macros::{action_dispatch, action_impl};
use brontes_types::normalized_actions::{NormalizedBurn, NormalizedMint, NormalizedSwap};
use reth_db::{mdbx::RO, transaction::DbTx};

use crate::SushiSwapV2::{burnCall, mintCall, swapCall};

action_impl!(
    V2SwapImpl,
    Swap,
    swapCall,
    [Ignore<Sync>, Swap],
    SushiSwapV2,
    call_data: true,
    logs: true,
    |trace_index,
    from_address: Address,
    target_address: Address,
    call_data: swapCall,
    logs: V2SwapImplSwap,
    db_tx: &LibmdbxTx<RO>| {
        let logs = logs.Swap_field;

        let recipient = call_data.to;
        let tokens = db_tx.get::<AddressToTokens>(target_address).ok()??;
        let [token_0, token_1] = [tokens.token0, tokens.token1];

        let amount_0_in: U256 = logs.amount0In;
        if amount_0_in == U256::ZERO {
            return Some(NormalizedSwap {
                pool: target_address,
                trace_index,
                from: from_address,
                recipient,
                token_in: token_1,
                token_out: token_0,
                amount_in: logs.amount1In,
                amount_out: logs.amount0Out,
            })
        } else {
            return Some(NormalizedSwap {
                trace_index,
                pool: target_address,
                from: from_address,
                recipient,
                token_in: token_0,
                token_out: token_1,
                amount_in: logs.amount0In,
                amount_out: logs.amount1Out,
            })
        }
    }
);

action_impl!(
    V2MintImpl,
    Mint,
    mintCall,
    // can be a double transfer if the pool has no liquidity
    [Possible<Ignore<Transfer>>, Ignore<Transfer>, Ignore<Sync>, Mint],
    SushiSwapV2,
    logs: true,
    call_data: true,
    |trace_index,
     from_address: Address,
     target_address: Address,
     call_data: mintCall,
     log_data: V2MintImplMint, db_tx: &LibmdbxTx<RO>| {
        let log_data = log_data.Mint_field;
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
    [Ignore<Transfer>, Ignore<Sync>, Burn],
    SushiSwapV2,
    call_data: true,
    logs: true,
    |trace_index,
     from_address: Address,
     target_address: Address,
     call_data: burnCall,
     log_data: V2BurnImplBurn, db_tx: &LibmdbxTx<RO>| {
        let log_data = log_data.Burn_field;
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

action_dispatch!(SushiSwapV2Classifier, V2SwapImpl, V2BurnImpl, V2MintImpl);
