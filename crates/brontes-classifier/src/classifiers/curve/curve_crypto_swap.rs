use brontes_macros::action_impl;
use brontes_types::{
    constants::{ETH_ADDRESS, WETH_ADDRESS},
    normalized_actions::NormalizedSwap,
    structured_trace::CallInfo,
    Protocol, ToScaledRational,
};
use reth_primitives::U256;

action_impl!(
    Protocol::CurveCryptoSwap,
    crate::CurveCryptoSwap::exchange_0Call,
    Swap,
    [TokenExchange],
    logs: true,
    |
    info: CallInfo,
    log: CurveCryptoSwapexchange_0CallLogs,
    db_tx: &DB| {
        let log = log.TokenExchange_field;

        let details = db_tx.get_protocol_details(info.target_address)?;
        let [token_0, token_1] = [details.token0, details.token1];

        let t0_info = db_tx.try_fetch_token_info(token_0)?;
        let t1_info = db_tx.try_fetch_token_info(token_1)?;

        if log.sold_id ==  U256::ZERO {
            let amount_in = log.tokens_sold.to_scaled_rational(t0_info.decimals);
            let amount_out = log.tokens_bought.to_scaled_rational(t1_info.decimals);
            return Ok(NormalizedSwap {
                protocol: Protocol::CurveCryptoSwap,
                pool: info.target_address,
                trace_index: info.trace_idx,
                from: info.from_address,
                recipient: info.from_address,
                token_in: t0_info,
                token_out: t1_info,
                amount_in,
                amount_out,
                msg_value: info.msg_value
            })
        } else {
            let amount_in = log.tokens_sold.to_scaled_rational(t1_info.decimals);
            let amount_out = log.tokens_bought.to_scaled_rational(t0_info.decimals);
            return Ok(NormalizedSwap {
                protocol: Protocol::CurveCryptoSwap,
                trace_index: info.trace_idx,
                pool: info.target_address,
                from: info.from_address,
                recipient: info.from_address,
                token_in: t1_info,
                token_out: t0_info,
                amount_in,
                amount_out,
                msg_value: info.msg_value
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
    |
    info: CallInfo,
    call_data: exchange_1Call,
    log: CurveCryptoSwapexchange_1CallLogs,
    db_tx: &DB| {

        let log = log.TokenExchange_field;
        let details = db_tx.get_protocol_details(info.target_address)?;
        let [mut token_0, mut token_1] = [details.token0, details.token1];

        let is_eth = call_data.use_eth;

        // Check if ETH_ADDRESS is used and adjust token_in or token_out accordingly
        if is_eth {
            if log.sold_id == U256::ZERO && token_0 == WETH_ADDRESS {
                token_0 = ETH_ADDRESS;
            } else if log.sold_id != U256::ZERO && token_1 == WETH_ADDRESS {
                token_1 = ETH_ADDRESS;
            }
        }

        let t0_info = db_tx.try_fetch_token_info(token_0)?;
        let t1_info = db_tx.try_fetch_token_info(token_1)?;

        if log.sold_id ==  U256::ZERO {
            let amount_in = log.tokens_sold.to_scaled_rational(t0_info.decimals);
            let amount_out = log.tokens_bought.to_scaled_rational(t1_info.decimals);
            return Ok(NormalizedSwap {
                protocol: Protocol::CurveCryptoSwap,
                pool: info.target_address,
                trace_index: info.trace_idx,
                from: info.from_address,
                recipient: info.from_address,
                token_in: t0_info,
                token_out: t1_info,
                amount_in,
                amount_out,
                msg_value: info.msg_value
            })
        } else {
            let amount_in = log.tokens_sold.to_scaled_rational(t1_info.decimals);
            let amount_out = log.tokens_bought.to_scaled_rational(t0_info.decimals);
            return Ok(NormalizedSwap {
                protocol: Protocol::CurveCryptoSwap,
                trace_index: info.trace_idx,
                pool: info.target_address,
                from: info.from_address,
                recipient: info.from_address,
                token_in: t1_info,
                token_out: t0_info,
                amount_in,
                amount_out,
                msg_value: info.msg_value
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
    |
    info: CallInfo,
    call_data: exchange_2Call,
    log: CurveCryptoSwapexchange_2CallLogs,
    db_tx: &DB| {

        let log = log.TokenExchange_field;
        let details = db_tx.get_protocol_details(info.target_address)?;
        let [mut token_0, mut token_1] = [details.token0, details.token1];
        let is_eth = call_data.use_eth;

        let _recipient = call_data.receiver;
        // Check if ETH_ADDRESS is used and adjust token_in or token_out accordingly
        if is_eth {
            if log.sold_id == U256::ZERO && token_0 == WETH_ADDRESS {
                token_0 = ETH_ADDRESS;
            } else if log.sold_id != U256::ZERO && token_1 == WETH_ADDRESS {
                token_1 = ETH_ADDRESS;
            }
        }

        let t0_info = db_tx.try_fetch_token_info(token_0)?;
        let t1_info = db_tx.try_fetch_token_info(token_1)?;

        if log.sold_id ==  U256::ZERO {
            let amount_in = log.tokens_sold.to_scaled_rational(t0_info.decimals);
            let amount_out = log.tokens_bought.to_scaled_rational(t1_info.decimals);
            return Ok(NormalizedSwap {
                protocol: Protocol::CurveCryptoSwap,
                pool: info.target_address,
                trace_index: info.trace_idx,
                from: info.from_address,
                recipient: info.from_address,
                token_in: t0_info,
                token_out: t1_info,
                amount_in,
                amount_out,
                msg_value: info.msg_value
            })
        } else {
            let amount_in = log.tokens_sold.to_scaled_rational(t1_info.decimals);
            let amount_out = log.tokens_bought.to_scaled_rational(t0_info.decimals);
            return Ok(NormalizedSwap {
                protocol: Protocol::CurveCryptoSwap,
                pool: info.target_address,
                trace_index: info.trace_idx,
                from: info.from_address,
                recipient: info.from_address,
                token_in: t1_info,
                token_out: t0_info,
                amount_in,
                amount_out,
                msg_value: info.msg_value
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
    |
    info: CallInfo,
    log: CurveCryptoSwapexchange_underlying_0CallLogs,
    db_tx: &DB| {
        let log = log.TokenExchange_field;
        let details = db_tx.get_protocol_details(info.target_address)?;
        let [mut token_0, mut token_1] = [details.token0, details.token1];


         // Replace WETH_ADDRESS with ETH_ADDRESS for token_in or token_out
         if token_0 == WETH_ADDRESS {
            token_0 = ETH_ADDRESS;
        }
        if token_1 == WETH_ADDRESS {
            token_1 = ETH_ADDRESS;
        }

        let t0_info = db_tx.try_fetch_token_info(token_0)?;
        let t1_info = db_tx.try_fetch_token_info(token_1)?;

        if log.sold_id ==  U256::ZERO {
            let amount_in = log.tokens_sold.to_scaled_rational(t0_info.decimals);
            let amount_out = log.tokens_bought.to_scaled_rational(t1_info.decimals);
            return Ok(NormalizedSwap {
                protocol: Protocol::CurveCryptoSwap,
                pool: info.target_address,
                trace_index: info.trace_idx,
                from: info.from_address,
                recipient: info.from_address,
                token_in: t0_info,
                token_out: t1_info,
                amount_in,
                amount_out,
                msg_value: info.msg_value
            })
        } else {
            let amount_in = log.tokens_sold.to_scaled_rational(t1_info.decimals);
            let amount_out = log.tokens_bought.to_scaled_rational(t0_info.decimals);
            return Ok(NormalizedSwap {
                protocol: Protocol::CurveCryptoSwap,
                pool: info.target_address,
                trace_index: info.trace_idx,
                from: info.from_address,
                recipient: info.from_address,
                token_in: t1_info,
                token_out: t0_info,
                amount_in,
                amount_out,
                msg_value: info.msg_value
            })
        }
    }
);
