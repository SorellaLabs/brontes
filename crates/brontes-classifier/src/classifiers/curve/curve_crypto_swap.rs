use alloy_primitives::{hex, FixedBytes};
use brontes_macros::action_impl;
use brontes_pricing::Protocol;
use brontes_types::{normalized_actions::NormalizedSwap, ToScaledRational};
use reth_primitives::{Address, U256};

pub const ETH: Address = Address(FixedBytes(hex!("EeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE")));
pub const WETH: Address = Address(FixedBytes(hex!("C02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2")));

action_impl!(
    Protocol::CurveCryptoSwap,
    crate::CurveCryptoSwap::exchange_0Call,
    Swap,
    [TokenExchange],
    logs: true,
    |trace_index,
    from_address: Address,
    target_address: Address,
    _msg_sender: Address,
    log: CurveCryptoSwapexchange_0CallLogs,
    db_tx: &DB| {
        let log = log.TokenExchange_field;

        let tokens = db_tx.get_protocol_tokens(target_address).ok()??;
        let [token_0, token_1] = [tokens.token0, tokens.token1];

        let t0_info = db_tx.try_get_token_info(token_0).ok()??;
        let t1_info = db_tx.try_get_token_info(token_1).ok()??;

        if log.sold_id ==  U256::ZERO {
            let amount_in = log.tokens_sold.to_scaled_rational(t0_info.decimals);
            let amount_out = log.tokens_bought.to_scaled_rational(t1_info.decimals);
            return Some(NormalizedSwap {
                protocol: Protocol::CurveCryptoSwap,
                pool: target_address,
                trace_index,
                from: from_address,
                recipient: from_address,
                token_in: t0_info,
                token_out: t1_info,
                amount_in,
                amount_out,
            })
        } else {
            let amount_in = log.tokens_sold.to_scaled_rational(t1_info.decimals);
            let amount_out = log.tokens_bought.to_scaled_rational(t0_info.decimals);
            return Some(NormalizedSwap {
                protocol: Protocol::CurveCryptoSwap,
                trace_index,
                pool: target_address,
                from: from_address,
                recipient: from_address,
                token_in: t1_info,
                token_out: t0_info,
                amount_in,
                amount_out,
            })
        }
    }
);

action_impl!(
    Protocol::CurveCryptoSwap,
    crate::CurveCryptoSwap::exchange_1Call,
    Swap,
    [TokenExchange],
    logs: true,
    call_data: true,
    |trace_index,
    from_address: Address,
    target_address: Address,
    _msg_sender: Address,
    call_data: exchange_1Call,
    log: CurveCryptoSwapexchange_1CallLogs,
    db_tx: &DB| {

        let log = log.TokenExchange_field;
        let tokens = db_tx.get_protocol_tokens(target_address).ok()??;
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

        let t0_info = db_tx.try_get_token_info(token_0).ok()??;
        let t1_info = db_tx.try_get_token_info(token_1).ok()??;

        if log.sold_id ==  U256::ZERO {
            let amount_in = log.tokens_sold.to_scaled_rational(t0_info.decimals);
            let amount_out = log.tokens_bought.to_scaled_rational(t1_info.decimals);
            return Some(NormalizedSwap {
                protocol: Protocol::CurveCryptoSwap,
                pool: target_address,
                trace_index,
                from: from_address,
                recipient: from_address,
                token_in: t0_info,
                token_out: t1_info,
                amount_in,
                amount_out,
            })
        } else {
            let amount_in = log.tokens_sold.to_scaled_rational(t1_info.decimals);
            let amount_out = log.tokens_bought.to_scaled_rational(t0_info.decimals);
            return Some(NormalizedSwap {
                protocol: Protocol::CurveCryptoSwap,
                trace_index,
                pool: target_address,
                from: from_address,
                recipient: from_address,
                token_in: t1_info,
                token_out: t0_info,
                amount_in,
                amount_out,
            })
        }
    }
);

action_impl!(
    Protocol::CurveCryptoSwap,
    crate::CurveCryptoSwap::exchange_2Call,
    Swap,
    [TokenExchange],
    logs: true,
    call_data: true,
    |trace_index,
    from_address: Address,
    target_address: Address,
    _msg_sender: Address,
    call_data: exchange_2Call,
    log: CurveCryptoSwapexchange_2CallLogs,
    db_tx: &DB| {

        let log = log.TokenExchange_field;
        let tokens = db_tx.get_protocol_tokens(target_address).ok()??;
        let [mut token_0, mut token_1] = [tokens.token0, tokens.token1];

        let is_eth = call_data.use_eth;

        let _recipient = call_data.receiver;
        // Check if ETH is used and adjust token_in or token_out accordingly
        if is_eth {
            if log.sold_id == U256::ZERO && token_0 == WETH {
                token_0 = ETH;
            } else if log.sold_id != U256::ZERO && token_1 == WETH {
                token_1 = ETH;
            }
        }

        let t0_info = db_tx.try_get_token_info(token_0).ok()??;
        let t1_info = db_tx.try_get_token_info(token_1).ok()??;

        if log.sold_id ==  U256::ZERO {
            let amount_in = log.tokens_sold.to_scaled_rational(t0_info.decimals);
            let amount_out = log.tokens_bought.to_scaled_rational(t1_info.decimals);
            return Some(NormalizedSwap {
                protocol: Protocol::CurveCryptoSwap,
                pool: target_address,
                trace_index,
                from: from_address,
                recipient: from_address,
                token_in: t0_info,
                token_out: t1_info,
                amount_in,
                amount_out,
            })
        } else {
            let amount_in = log.tokens_sold.to_scaled_rational(t1_info.decimals);
            let amount_out = log.tokens_bought.to_scaled_rational(t0_info.decimals);
            return Some(NormalizedSwap {
                protocol: Protocol::CurveCryptoSwap,
                trace_index,
                pool: target_address,
                from: from_address,
                recipient: from_address,
                token_in: t1_info,
                token_out: t0_info,
                amount_in,
                amount_out,
            })
        }
    }
);

// I don't know who coded this contract, but I wish them great harm.
action_impl!(
    Protocol::CurveCryptoSwap,
    crate::CurveCryptoSwap::exchange_underlying_0Call,
    Swap,
    [TokenExchange],
    logs: true,
    |trace_index,
    from_address: Address,
    target_address: Address,
    _msg_sender: Address,
    log: CurveCryptoSwapexchange_underlying_0CallLogs,
    db_tx: &DB| {
        let log = log.TokenExchange_field;
        let tokens = db_tx.get_protocol_tokens(target_address).ok()??;
        let [mut token_0, mut token_1] = [tokens.token0, tokens.token1];


         // Replace WETH with ETH for token_in or token_out
         if token_0 == WETH {
            token_0 = ETH;
        }
        if token_1 == WETH {
            token_1 = ETH;
        }

        let t0_info = db_tx.try_get_token_info(token_0).ok()??;
        let t1_info = db_tx.try_get_token_info(token_1).ok()??;

        if log.sold_id ==  U256::ZERO {
            let amount_in = log.tokens_sold.to_scaled_rational(t0_info.decimals);
            let amount_out = log.tokens_bought.to_scaled_rational(t1_info.decimals);
            return Some(NormalizedSwap {
                protocol: Protocol::CurveCryptoSwap,
                pool: target_address,
                trace_index,
                from: from_address,
                recipient: from_address,
                token_in: t0_info,
                token_out: t1_info,
                amount_in,
                amount_out,
            })
        } else {
            let amount_in = log.tokens_sold.to_scaled_rational(t1_info.decimals);
            let amount_out = log.tokens_bought.to_scaled_rational(t0_info.decimals);
            return Some(NormalizedSwap {
                protocol: Protocol::CurveCryptoSwap,
                trace_index,
                pool: target_address,
                from: from_address,
                recipient: from_address,
                token_in: t1_info,
                token_out: t0_info,
                amount_in,
                amount_out,
            })
        }
    }
);
