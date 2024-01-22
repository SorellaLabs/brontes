use alloy_primitives::{Address, U256};
use brontes_database::libmdbx::{tables::AddressToTokens, tx::CompressedLibmdbxTx};
use brontes_macros::{action_dispatch, action_impl};
use brontes_pricing::Protocol;
use brontes_types::normalized_actions::{
    NormalizedBurn, NormalizedCollect, NormalizedMint, NormalizedSwap,
};
use reth_db::mdbx::RO;

use crate::SushiSwapV3::{
    burnCall, burnReturn, collectCall, collectReturn, mintCall, mintReturn, swapCall, swapReturn,
};

action_impl!(
    Protocol::SushiSwapV3,
    Swap,
    swapCall,
    [Swap],
    SushiSwapV3,
    call_data: true,
    return_data: true,
    |trace_index,
    from_address: Address,
    target_address: Address,
     msg_sender: Address,
    call_data: swapCall,
    return_data: swapReturn,
    db_tx: &CompressedLibmdbxTx<RO>| {
        let token_0_delta = return_data.amount0;
        let token_1_delta = return_data.amount1;
        let recipient = call_data.recipient;
        let tokens = db_tx.get::<AddressToTokens>(target_address).ok()??;
        let [token_0, token_1] = [tokens.token0, tokens.token1];
        let (amount_in, amount_out, token_in, token_out) = if token_0_delta.is_negative() {
            (
                U256::from_be_bytes(token_1_delta.to_be_bytes::<32>()),
                U256::from_be_bytes(token_0_delta.abs().to_be_bytes::<32>()),
                token_1,
                token_0,
            )
        } else {
            (
                U256::from_be_bytes(token_0_delta.to_be_bytes::<32>()),
                U256::from_be_bytes(token_1_delta.abs().to_be_bytes::<32>()),
                token_0,
                token_1,
            )
        };

        Some(NormalizedSwap {
            trace_index,
            from: from_address,
            recipient,
            pool: target_address,
            token_in,
            token_out,
            amount_in,
            amount_out,
        })
    }
);
action_impl!(
    Protocol::SushiSwapV3,
    Mint,
    mintCall,
    [Mint],
    SushiSwapV3,
    return_data: true,
    call_data: true,
    |trace_index,
     from_address: Address,
     target_address: Address,
     msg_sender: Address,
     call_data: mintCall,
     return_data: mintReturn,  db_tx: &CompressedLibmdbxTx<RO>| {
        let token_0_delta = return_data.amount0;
        let token_1_delta = return_data.amount1;
        let tokens = db_tx.get::<AddressToTokens>(target_address).ok()??;
        let [token_0, token_1] = [tokens.token0, tokens.token1];

        Some(NormalizedMint {
            trace_index,
            from: from_address,
            recipient: call_data.recipient,
            to: target_address,
            token: vec![token_0, token_1],
            amount: vec![token_0_delta, token_1_delta],
        })
    }
);
action_impl!(
    Protocol::SushiSwapV3,
    Burn,
    burnCall,
    [Burn],
    SushiSwapV3,
    return_data: true,
    |trace_index,
    from_address: Address,
    target_address: Address,
     msg_sender: Address,
    return_data: burnReturn,
    db_tx: &CompressedLibmdbxTx<RO>| {
        let token_0_delta = return_data.amount0;
        let token_1_delta = return_data.amount1;

        let token_0_delta: U256 = return_data.amount0;
        let token_1_delta: U256 = return_data.amount1;
        let tokens = db_tx.get::<AddressToTokens>(target_address).ok()??;
        let [token_0, token_1] = [tokens.token0, tokens.token1];

        Some(NormalizedBurn {
            to: target_address,
            recipient: target_address,
            trace_index,
            from: from_address,
            token: vec![token_0, token_1],
            amount: vec![token_0_delta, token_1_delta],
        })
    }
);
action_impl!(
    Protocol::SushiSwapV3,
    Collect,
    collectCall,
    [Collect],
    SushiSwapV3,
    call_data: true,
    return_data: true,
    |
    trace_index,
    from_addr: Address,
    to_addr: Address,
     msg_sender: Address,
    call_data: collectCall,
    return_data: collectReturn,  db_tx: &CompressedLibmdbxTx<RO>
    | {
        let tokens = db_tx.get::<AddressToTokens>(target_address).ok()??;
        let [token_0, token_1] = [tokens.token0, tokens.token1];
        Some(NormalizedCollect {
            trace_index,
            from: from_addr,
            recipient: call_data.recipient,
            to: to_addr,
            token: vec![token_0, token_1],
            amount: vec![U256::from(return_data.amount0), U256::from(return_data.amount1)],
        })
    }
);
