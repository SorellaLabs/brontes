use alloy_sol_types::SolCall;
use brontes_macros::{action_dispatch, action_impl};
use brontes_types::normalized_actions::{
    Actions, NormalizedBurn, NormalizedCollect, NormalizedMint, NormalizedSwap,
};
use reth_primitives::{Address, Bytes, H160, U256};
use reth_rpc_types::Log;

use crate::{
    enum_unwrap, ActionCollection, IntoAction, StaticReturnBindings,
    UniswapV3::{
        burnCall, burnReturn, collectCall, collectReturn, mintCall, mintReturn, swapCall,
        swapReturn, UniswapV3Calls,
    },
    ADDRESS_TO_TOKENS_2_POOL,
};

action_impl!(
    V3SwapImpl,
    Swap,
    swapCall,
    None,
    false,
    true,
    |index, from_address: H160, target_address: H160, return_data: swapReturn| {
        let address_bytes: [u8; 20] = target_address.clone().0.try_into().unwrap();
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

        Some(NormalizedSwap {
            index,
            from: from_address,
            pool: target_address,
            token_in,
            token_out,
            amount_in,
            amount_out,
        })
    }
);

action_impl!(
    V3MintImpl,
    Mint,
    mintCall,
    Some(UniswapV3),
    false,
    true,
    |index,
     from_address: H160,
     target_address: H160,
     call_data: mintCall,
     return_data: mintReturn| {
        let address_bytes: [u8; 20] = target_address.clone().0.try_into().unwrap();
        let token_0_delta = return_data.amount0;
        let token_1_delta = return_data.amount1;
        let [token0, token1] = ADDRESS_TO_TOKENS_2_POOL
            .get(&address_bytes)
            .copied()
            .unwrap();

        Some(NormalizedMint {
            index,
            from: from_address,
            recipient: H160(call_data.recipient.0 .0),
            to: target_address,
            token: vec![token0, token1],
            amount: vec![token_0_delta, token_1_delta],
        })
    }
);

action_impl!(
    V3BurnImpl,
    Burn,
    burnCall,
    None,
    false,
    true,
    |index, from_address: H160, target_address: H160, return_data: burnReturn| {
        let address_bytes: [u8; 20] = target_address.clone().0.try_into().unwrap();
        let token_0_delta = return_data.amount0;
        let token_1_delta = return_data.amount1;

        let token_0_delta: U256 = return_data.amount0;
        let token_1_delta: U256 = return_data.amount1;
        let [token_0, token_1] = ADDRESS_TO_TOKENS_2_POOL
            .get(&address_bytes)
            .copied()
            .unwrap();

        Some(NormalizedBurn {
            to: target_address,
            recipient: target_address,
            index,
            from: from_address,
            token: vec![token_0, token_1],
            amount: vec![token_0_delta, token_1_delta],
        })
    }
);

action_impl!(
    V3CollectImpl,
    Collect,
    collectCall,
    Some(UniswapV3),
    false,
    true,
    |index, from_addr: H160, to_addr: H160, call_data: collectCall, return_data: collectReturn| {
        let address_bytes: [u8; 20] = target_address.clone().0.try_into().unwrap();
        let [token0, token1] = ADDRESS_TO_TOKENS_2_POOL
            .get(&address_bytes)
            .copied()
            .unwrap();
        Some(NormalizedCollect {
            index,
            from: from_addr,
            recipient: H160(call_data.recipient.0 .0),
            to: to_addr,
            token: vec![token0, token1],
            amount: vec![U256::from(return_data.amount0), U256::from(return_data.amount1)],
        })
    }
);

action_dispatch!(UniswapV3Classifier, V3SwapImpl, V3BurnImpl, V3MintImpl, V3CollectImpl);
