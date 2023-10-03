use alloy_sol_types::SolCall;
use brontes_core::{
    StaticReturnBindings,
    UniswapV3::{
        burnCall, burnReturn, collectCall, collectReturn, mintCall, mintReturn, swapCall,
        swapReturn, UniswapV3Calls,
    },
};
use brontes_types::normalized_actions::{
    Actions, NormalizedBurn, NormalizedCollect, NormalizedMint, NormalizedSwap,
};
use reth_primitives::{Address, Bytes, H160, U256};
use reth_rpc_types::Log;

use crate::{
    action_impl_calldata, action_impl_return, enum_unwrap, IntoAction, ADDRESS_TO_TOKENS_2_POOL,
};

action_impl_return!(
    V3SwapImpl,
    Swap,
    swapCall,
    |index, from_address: H160, target_address: H160, return_data: swapReturn| {
        let token_0_delta = return_data.amount0;
        let token_1_delta = return_data.amount1;
        let [token_0, token_1] = ADDRESS_TO_TOKENS_2_POOL
            .get(&*target_address)
            .copied()
            .unwrap();
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
            from: from_address,
            pool: target_address,
            token_in,
            token_out,
            amount_in,
            amount_out,
        }
    }
);

action_impl_return!(
    V3BurnImpl,
    Burn,
    burnCall,
    |index, from_address: H160, target_address: H160, return_data: burnReturn| {
        let token_0_delta: U256 = return_data.amount0;
        let token_1_delta: U256 = return_data.amount1;
        let [token_0, token_1] = ADDRESS_TO_TOKENS_2_POOL
            .get(&*target_address)
            .copied()
            .unwrap();

        NormalizedBurn {
            to: target_address,
            recipient: target_address,
            index,
            from: from_address,
            token: vec![token_0, token_1],
            amount: vec![token_0_delta, token_1_delta],
        }
    }
);

action_impl_return!(
    V3MintImpl,
    Mint,
    mintCall,
    |index, from_address: H160, target_address: H160, return_data: mintReturn| {
        let token_0_delta = return_data.amount0;
        let token_1_delta = return_data.amount1;
        let [token0, token1] = ADDRESS_TO_TOKENS_2_POOL
            .get(&*from_address)
            .copied()
            .unwrap();

        // todo this address shit wrong but wanna build
        NormalizedMint {
            index,
            from: target_address,
            recipient: from_address,
            to: target_address,
            token: vec![token0, token1],
            amount: vec![token_0_delta, token_1_delta],
        }
    }
);

action_impl_calldata!(
    V3CollectImpl,
    Collect,
    collectCall,
    UniswapV3,
    |index, from_addr: H160, to_addr: H160, call_data: collectCall, return_data: collectReturn| {
        let [token0, token1] = ADDRESS_TO_TOKENS_2_POOL.get(&*to_addr).copied().unwrap();
        NormalizedCollect {
            index,
            from: from_addr,
            recipient: from_addr,
            to: to_addr,
            token: vec![token0, token1],
            amount: vec![U256::from(return_data.amount0), U256::from(return_data.amount1)],
        }
    }
);
