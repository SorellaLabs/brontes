use alloy_sol_types::SolCall;
use brontes_core::{
    StaticReturnBindings,
    UniswapV3::{burnCall, burnReturn, mintCall, mintReturn, swapCall, swapReturn},
};
use brontes_types::normalized_actions::{Actions, NormalizedBurn, NormalizedMint, NormalizedSwap};
use reth_primitives::{Address, Bytes, H160, U256};
use reth_rpc_types::Log;

use crate::{action_impl_return, IntoAction, ADDRESS_TO_TOKENS_2_POOL};

action_impl_return!(V3SwapImpl, Swap, swapCall, |index, address: H160, return_data: swapReturn| {
    let token_0_delta = return_data.amount0;
    let token_1_delta = return_data.amount1;
    let [token_0, token_1] = ADDRESS_TO_TOKENS_2_POOL.get(&*address).copied().unwrap();
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

    NormalizedSwap {
        index,
        from: address,
        pool: address,
        token_in,
        token_out,
        amount_in,
        amount_out,
    }
});

action_impl_return!(V3BurnImpl, Burn, burnCall, |index, address: H160, return_data: burnReturn| {
    let token_0_delta: U256 = return_data.amount0;
    let token_1_delta: U256 = return_data.amount1;
    let [token_0, token_1] = ADDRESS_TO_TOKENS_2_POOL.get(&*address).copied().unwrap();

    NormalizedBurn {
        to: address,
        recipient: address,
        index,
        from: address,
        token: vec![token_0, token_1],
        amount: vec![token_0_delta, token_1_delta],
    }
});

action_impl_return!(V3MintImpl, Mint, mintCall, |index, address: H160, return_data: mintReturn| {
    let token_0_delta = return_data.amount0;
    let token_1_delta = return_data.amount1;
    let [token0, token1] = ADDRESS_TO_TOKENS_2_POOL.get(&*address).copied().unwrap();

    NormalizedMint {
        index,
        from: address,
        recipient: address,
        to: address,
        token: vec![token0, token1],
        amount: vec![token_0_delta, token_1_delta],
    }
});
