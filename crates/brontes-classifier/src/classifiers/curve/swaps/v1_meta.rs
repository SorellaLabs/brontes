use alloy_primitives::U256;
use brontes_macros::action_impl;
use brontes_pricing::Protocol;
use brontes_types::{
    constants::{ETH_ADDRESS, WETH_ADDRESS},
    normalized_actions::NormalizedSwap,
    structured_trace::CallInfo,
    ToScaledRational,
};

// CurveV1Metapool::exchange_0Call delegates to the implementation contract

// could not find a v1 metapool calling this
action_impl!(
    Protocol::CurveV1MetaPool,
    crate::CurveV1Metapool::exchange_1Call,
    Swap,
    [..TokenExchange],
    logs: true,
    |
    info: CallInfo,
    log: CurveV1MetaPoolexchange_1CallLogs,
    db_tx: &DB|{
        let log = log.TokenExchange_field;

        let details = db_tx.get_protocol_details(info.target_address)?;

        let token_in_addr = match log.sold_id {
            0 => details.token0,
            1 => details.token1,
            2 => details.token2.ok_or(eyre::eyre!("Expected token2 for token in, found None"))?,
            3 => details.token3.ok_or(eyre::eyre!("Expected token3 for token in, found None"))?,
            4 => details.token4.ok_or(eyre::eyre!("Expected token4 for token in, found None"))?,
            _ => unreachable!()
        };

        let token_out_addr = match log.bought_id {
            0 => details.token0,
            1 => details.token1,
            2 => details.token2.ok_or(eyre::eyre!("Expected token2 for token out, found None"))?,
            3 => details.token3.ok_or(eyre::eyre!("Expected token3 for token out, found None"))?,
            4 => details.token4.ok_or(eyre::eyre!("Expected token4 for token out, found None"))?,
            _ => unreachable!()
        };

        let token_in = db_tx.try_fetch_token_info(token_in_addr)?;
        let token_out = db_tx.try_fetch_token_info(token_out_addr)?;

        let amount_in = log.tokens_sold.to_scaled_rational(token_in.decimals);
        let amount_out = log.tokens_bought.to_scaled_rational(token_out.decimals);


        Ok(NormalizedSwap {
            protocol: Protocol::CurveV1MetaPool,
            trace_index: info.trace_idx,
            pool: info.target_address,
            from: info.from_address,
            recipient: info.from_address,
            token_in,
            token_out,
            amount_in,
            amount_out,
            msg_value: info.msg_value
        })
    }
);

// could not find a v1 metapool calling this
action_impl!(
    Protocol::CurveV1MetaPool,
    crate::CurveV1Metapool::exchange_underlying_1Call,
    Swap,
    [..TokenExchangeUnderlying],
    logs: true,
    |
    info: CallInfo,
    log: CurveV1MetaPoolexchange_underlying_1CallLogs,
    db_tx: &DB|{
        let log = log.TokenExchangeUnderlying_field;

        let details = db_tx.get_protocol_details(info.target_address)?;

        let token_in_addr = match log.sold_id {
            0 => details.token0,
            1 => details.token1,
            2 => details.token2.ok_or(eyre::eyre!("Expected token2 for token in, found None"))?,
            3 => details.token3.ok_or(eyre::eyre!("Expected token3 for token in, found None"))?,
            4 => details.token4.ok_or(eyre::eyre!("Expected token4 for token in, found None"))?,
            _ => unreachable!()
        };

        let token_out_addr = match log.bought_id {
            0 => details.token0,
            1 => details.token1,
            2 => details.token2.ok_or(eyre::eyre!("Expected token2 for token out, found None"))?,
            3 => details.token3.ok_or(eyre::eyre!("Expected token3 for token out, found None"))?,
            4 => details.token4.ok_or(eyre::eyre!("Expected token4 for token out, found None"))?,
            _ => unreachable!()
        };

        let token_in = db_tx.try_fetch_token_info(token_in_addr)?;
        let token_out = db_tx.try_fetch_token_info(token_out_addr)?;

        let amount_in = log.tokens_sold.to_scaled_rational(token_in.decimals);
        let amount_out = log.tokens_bought.to_scaled_rational(token_out.decimals);


        Ok(NormalizedSwap {
            protocol: Protocol::CurveV1MetaPool,
            trace_index: info.trace_idx,
            pool: info.target_address,
            from: info.from_address,
            recipient: info.from_address,
            token_in,
            token_out,
            amount_in,
            amount_out,
            msg_value: info.msg_value
        })
    }
);
