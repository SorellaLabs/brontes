use alloy_sol_types::{SolCall, SolEvent};
use brontes_macros::{action_dispatch, action_impl};
use brontes_types::normalized_actions::{Actions, NormalizedBurn, NormalizedMint, NormalizedSwap};
use reth_primitives::{Address, Bytes, U256};
use reth_rpc_types::Log;

use crate::{
    enum_unwrap, ActionCollection,
    CurveCryptoSwap::{exchange_0Call, exchange_1Call, exchange_2Call, TokenExchange},
    IntoAction, StaticReturnBindings, ADDRESS_TO_TOKENS_2_POOL,
};

action_impl!(
    CurveCryptoExchange0,
    Exchange0,
    exchange_0Call,
    CurveCryptoSwap,
    logs: true,
    |index, from_address: Address, target_address: Address, log_data: Option<TokenExchange> | {
        let log = log_data?;
        let [token_0, token_1] = ADDRESS_TO_TOKENS_2_POOL.get(&*target_address.0).copied()?;


        if log.sold_id ==  U256::ZERO {
            return Some(NormalizedSwap {
                pool: target_address,
                index,
                from: from_address,
                token_in: token_0,
                token_out: token_1,
                amount_in: log.tokens_sold,
                amount_out: log.tokens_bought,
            })
        } else {
            return Some(NormalizedSwap {
                index,
                pool: target_address,
                from: from_address,
                token_in: token_1,
                token_out: token_0,
                amount_in: log.tokens_sold,
                amount_out: log.tokens_bought,
            })
        }
    }
);

action_impl!(
    CurveCryptoExchange1,
    Exchange1,
    exchange_1Call,
    CurveCryptoSwap,
    logs: true,
    |index, from_address: Address, target_address: Address, log_data: Option<TokenExchange> | {
        let log = log_data?;
        let [token_0, token_1] = ADDRESS_TO_TOKENS_2_POOL.get(&*target_address.0).copied()?;

        if log.sold_id ==  U256::ZERO {
            return Some(NormalizedSwap {
                pool: target_address,
                index,
                from: from_address,
                token_in: token_0,
                token_out: token_1,
                amount_in: log.tokens_sold,
                amount_out: log.tokens_bought,
            })
        } else {
            return Some(NormalizedSwap {
                index,
                pool: target_address,
                from: from_address,
                token_in: token_1,
                token_out: token_0,
                amount_in: log.tokens_sold,
                amount_out: log.tokens_bought,
            })
        }
    }
);

/// I don't know who coded this contract, but I wish them great harm.
action_impl!(
CurveCryptoExchangeUnderlying,
Exchangeunderlying,
exchange_underlying_Call,
CurveCryptoSwap,
logs: true,
|index, from_address: Address, target_address: Address, log_data: Option<TokenExchange> | {
        let log = log_data?;
        let [token_0, token_1] = ADDRESS_TO_TOKENS_2_POOL.get(&*target_address.0).copied()?;


        if log.sold_id ==  U256::ZERO {
            return Some(NormalizedSwap {
                pool: target_address,
                index,
                from: from_address,
                token_in: token_0,
                token_out: token_1,
                amount_in: log.tokens_sold,
                amount_out: log.tokens_bought,
            })
        } else {
            return Some(NormalizedSwap {
                index,
                pool: target_address,
                from: from_address,
                token_in: token_1,
                token_out: token_0,
                amount_in: log.tokens_sold,
                amount_out: log.tokens_bought,
            })
        }
    }
);

action_dispatch!(
    CurveCryptoSwapClassifier,
    CurveCryptoExchange0,
    CurveCryptoExchange1,
    CurveCryptoExchangeUnderlying
);
