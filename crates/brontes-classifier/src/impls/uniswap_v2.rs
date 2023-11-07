use alloy_sol_types::{SolCall, SolEvent};
use brontes_macros::{action_dispatch, action_impl};
use brontes_types::normalized_actions::{Actions, NormalizedBurn, NormalizedMint, NormalizedSwap};
use reth_primitives::{Address, Bytes, H160, U256};
use reth_rpc_types::Log;

use crate::{
    enum_unwrap, ActionCollection, IntoAction, StaticReturnBindings,
    UniswapV2::{burnCall, mintCall, swapCall, Burn, Mint, Swap, UniswapV2Calls},
    ADDRESS_TO_TOKENS_2_POOL,
};

action_impl!(
    V2SwapImpl,
    Swap,
    swapCall,
    None,
    true,
    false,
    |index, from_address: H160, target_address: H160, data: Option<Swap>| {
        let data = data?;
        let address_bytes: [u8; 20] = target_address.clone().0.try_into().unwrap();
        let [token_0, token_1] = ADDRESS_TO_TOKENS_2_POOL.get(&address_bytes).copied()?;
        let amount_0_in: U256 = data.amount0In;
        if amount_0_in == U256::ZERO {
            return Some(NormalizedSwap {
                pool: target_address,
                index,
                from: from_address,
                token_in: token_1,
                token_out: token_0,
                amount_in: data.amount1In,
                amount_out: data.amount0Out,
            })
        } else {
            return Some(NormalizedSwap {
                index,
                pool: target_address,
                from: from_address,
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
    Some(UniswapV2),
    true,
    false,
    |index,
     from_address: H160,
     target_address: H160,
     call_data: mintCall,
     log_data: Option<Mint>| {
        let address_bytes: [u8; 20] = target_address.clone().0.try_into().unwrap();
        let log_data = log_data?;
        let [token_0, token_1] = ADDRESS_TO_TOKENS_2_POOL.get(&address_bytes).copied()?;
        Some(NormalizedMint {
            recipient: H160(call_data.to.0 .0),
            from: from_address,
            index,
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
    Some(UniswapV2),
    true,
    false,
    |index,
     from_address: H160,
     target_address: H160,
     call_data: burnCall,
     log_data: Option<Burn>| {
        let address_bytes: [u8; 20] = target_address.clone().0.try_into().unwrap();
        let log_data = log_data?;
        let [token_0, token_1] = ADDRESS_TO_TOKENS_2_POOL.get(&address_bytes).copied()?;
        Some(NormalizedBurn {
            recipient: H160(call_data.to.0 .0),
            to: target_address,
            index,
            from: from_address,
            token: vec![token_0, token_1],
            amount: vec![log_data.amount0, log_data.amount1],
        })
    }
);

action_dispatch!(UniswapV2Classifier, V2SwapImpl, V2BurnImpl, V2MintImpl);
action_dispatch!(SushiSwapV2Classifier, V2SwapImpl, V2BurnImpl, V2MintImpl);
