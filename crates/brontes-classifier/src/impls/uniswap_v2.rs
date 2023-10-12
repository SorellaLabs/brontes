use alloy_sol_types::{SolCall, SolEvent};
use brontes_macros::{action_dispatch, action_impl};
use brontes_types::normalized_actions::{Actions, NormalizedBurn, NormalizedMint, NormalizedSwap};
use reth_primitives::{Address, Bytes, H160, U256};
use reth_rpc_types::Log;

use crate::{
    ActionCollection, IntoAction, StaticReturnBindings,
    SushiSwapV2::{burnCall, mintCall, swapCall, Burn, Mint, Swap},
    ADDRESS_TO_TOKENS_2_POOL,
};

action_impl!(
    V2SwapImpl,
    Swap,
    swapCall,
    None,
    true,
    false,
    |index, from_address: H160, target_address: H160, data: Swap| {
        let [token_0, token_1] = ADDRESS_TO_TOKENS_2_POOL
            .get(&*from_address)
            .copied()
            .unwrap();
        let amount_0_in: U256 = data.amount0In;
        if amount_0_in == U256::ZERO {
            return NormalizedSwap {
                pool: target_address,
                index,
                from: from_address,
                token_in: token_1,
                token_out: token_0,
                amount_in: data.amount1In,
                amount_out: data.amount0Out,
            }
        } else {
            return NormalizedSwap {
                index,
                pool: target_address,
                from: from_address,
                token_in: token_0,
                token_out: token_1,
                amount_in: data.amount0In,
                amount_out: data.amount1Out,
            }
        }
    }
);
// action_impl_log_no_return!(V2SwapImpl, Swap, swapCall,);

action_impl!(
    V2MintImpl,
    Mint,
    mintCall,
    None,
    true,
    false,
    |index, from_address: H160, target_address: H160, data: Mint| {
        let [token_0, token_1] = ADDRESS_TO_TOKENS_2_POOL
            .get(&*target_address)
            .copied()
            .unwrap();
        NormalizedMint {
            recipient: from_address,
            from: from_address,
            index,
            // todo fix
            to: H160(data.sender.0 .0),
            token: vec![token_0, token_1],
            amount: vec![data.amount0, data.amount1],
        }
    }
);

action_impl!(
    V2BurnImpl,
    Burn,
    burnCall,
    None,
    true,
    false,
    |index, from_address: H160, target_address: H160, res: Burn| {
        let [token_0, token_1] = ADDRESS_TO_TOKENS_2_POOL
            .get(&*target_address)
            .copied()
            .unwrap();

        NormalizedBurn {
            recipient: from_address,
            to: target_address,
            index,
            from: from_address,
            token: vec![token_0, token_1],
            amount: vec![res.amount0, res.amount1],
        }
    }
);

action_dispatch!(UniswapV2Classifier, V2SwapImpl, V2BurnImpl, V2MintImpl);
action_dispatch!(SushiSwapV2Classifier, V2SwapImpl, V2BurnImpl, V2MintImpl);
