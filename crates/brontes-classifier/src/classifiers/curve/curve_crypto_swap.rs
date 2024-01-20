use alloy_primitives::{hex, FixedBytes};
use brontes_database_libmdbx::{implementation::tx::LibmdbxTx, tables::AddressToTokens};
use brontes_macros::{action_dispatch, action_impl};
use brontes_types::normalized_actions::NormalizedSwap;
use reth_db::{mdbx::RO, transaction::DbTx};
use reth_primitives::{Address, U256};

use crate::CurveCryptoSwap::{
    exchange_0Call, exchange_1Call, exchange_2Call, exchange_underlying_0Call,
};

pub const ETH: Address = Address(FixedBytes(hex!("EeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE")));
pub const WETH: Address = Address(FixedBytes(hex!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2")));

action_impl!(
    CurveCryptoExchange0,
    Swap,
    exchange_0Call,
    [TokenExchange],
    CurveCryptoSwap,
    logs: true,
    |trace_index,
    from_address: Address,
    target_address: Address,
    msg_sender: Address,
    log: CurveCryptoExchange0Swap,
    db_tx: &LibmdbxTx<RO>| {
        let log = log.TokenExchange_field;
        let tokens = db_tx.get::<AddressToTokens>(target_address).ok()??;
        let [mut token_0, mut token_1] = [tokens.token0, tokens.token1];

        if log.sold_id ==  U256::ZERO {
            return Some(NormalizedSwap {
                pool: target_address,
                trace_index,
                from: from_address,
                recipient: from_address,
                token_in: token_0,
                token_out: token_1,
                amount_in: log.tokens_sold,
                amount_out: log.tokens_bought,
            })
        } else {
            return Some(NormalizedSwap {
                trace_index,
                pool: target_address,
                from: from_address,
                recipient: from_address,
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
    Swap,
    exchange_1Call,
    [TokenExchange],
    CurveCryptoSwap,
    logs: true,
    call_data: true,
    |trace_index,
    from_address: Address,
    target_address: Address,
    msg_sender: Address,
    call_data: exchange_1Call,
    log: CurveCryptoExchange1Swap,
    db_tx: &LibmdbxTx<RO>| {

        let log = log.TokenExchange_field;
        let tokens = db_tx.get::<AddressToTokens>(target_address).ok()??;
        let [mut token_0, mut token_1] = [tokens.token0, tokens.token1];

        let is_eth = call_data.use_eth;

        // Check if ETH is used and adjust token_in or token_out accordingly
        if is_eth {
            if log.sold_id == U256::ZERO && token_0 == WETH {
                token_0 = ETH;
            } else if log.sold_id != U256::ZERO && token_1 == WETH {
                token_1 = ETH;
            }
        }

        if log.sold_id == U256::ZERO {
            return Some(NormalizedSwap {
                pool: target_address,
                trace_index,
                from: from_address,
                recipient: from_address,
                token_in: token_0,
                token_out: token_1,
                amount_in: log.tokens_sold,
                amount_out: log.tokens_bought,
            })
        } else {
            return Some(NormalizedSwap {
                trace_index,
                pool: target_address,
                from: from_address,
                recipient: from_address,
                token_in: token_1,
                token_out: token_0,
                amount_in: log.tokens_sold,
                amount_out: log.tokens_bought,
            })
        }
    }
);

action_impl!(
    CurveCryptoExchange2,
    Swap,
    exchange_2Call,
    [TokenExchange],
    CurveCryptoSwap,
    logs: true,
    call_data: true,
    |trace_index,
    from_address: Address,
    target_address: Address,
    msg_sender: Address,
    call_data: exchange_2Call,
    log: CurveCryptoExchange2Swap,
    db_tx: &LibmdbxTx<RO>| {

        let log = log.TokenExchange_field;
        let tokens = db_tx.get::<AddressToTokens>(target_address).ok()??;
        let [mut token_0, mut token_1] = [tokens.token0, tokens.token1];

        let is_eth = call_data.use_eth;

        let recipient = call_data.receiver;
        // Check if ETH is used and adjust token_in or token_out accordingly
        if is_eth {
            if log.sold_id == U256::ZERO && token_0 == WETH {
                token_0 = ETH;
            } else if log.sold_id != U256::ZERO && token_1 == WETH {
                token_1 = ETH;
            }
        }

        if log.sold_id == U256::ZERO {
            return Some(NormalizedSwap {
                pool: target_address,
                trace_index,
                from: from_address,
                recipient,
                token_in: token_0,
                token_out: token_1,
                amount_in: log.tokens_sold,
                amount_out: log.tokens_bought,
            })
        } else {
            return Some(NormalizedSwap {
                trace_index,
                pool: target_address,
                from: from_address,
                recipient,
                token_in: token_1,
                token_out: token_0,
                amount_in: log.tokens_sold,
                amount_out: log.tokens_bought,
            })
        }
    }
);

// I don't know who coded this contract, but I wish them great harm.
action_impl!(
    CurveCryptoExchangeUnderlying,
    Swap,
    exchange_underlying_0Call,
    [TokenExchange],
    CurveCryptoSwap,
    logs: true,
    |trace_index,
    from_address: Address,
    target_address: Address,
    msg_sender: Address,
    log: CurveCryptoExchangeUnderlyingSwap,
    db_tx: &LibmdbxTx<RO>| {
        let log = log.TokenExchange_field;
        let tokens = db_tx.get::<AddressToTokens>(target_address).ok()??;
        let [mut token_0, mut token_1] = [tokens.token0, tokens.token1];


         // Replace WETH with ETH for token_in or token_out
         if token_0 == WETH {
            token_0 = ETH;
        }
        if token_1 == WETH {
            token_1 = ETH;
        }

        if log.sold_id ==  U256::ZERO {
            return Some(NormalizedSwap {
                pool: target_address,
                trace_index,
                from: from_address,
                recipient: from_address,
                token_in: token_0,
                token_out: token_1,
                amount_in: log.tokens_sold,
                amount_out: log.tokens_bought,
            })
        } else {
            return Some(NormalizedSwap {
                trace_index,
                pool: target_address,
                from: from_address,
                recipient: from_address,
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
