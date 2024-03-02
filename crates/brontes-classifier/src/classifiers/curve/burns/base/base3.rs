use brontes_macros::action_impl;
use brontes_pricing::Protocol;
use brontes_types::{
    normalized_actions::NormalizedBurn, structured_trace::CallInfo, ToScaledRational,
};

action_impl!(
    Protocol::CurveBasePool3,
    crate::CurveBase3::remove_liquidityCall,
    Burn,
    [..RemoveLiquidity],
    logs: true,
    |
    info: CallInfo,
    log: CurveBasePool3Remove_liquidityCallLogs,
    db_tx: &DB
    |{
        let log = log.remove_liquidity_field?;
        let details = db_tx.get_protocol_details(info.target_address)?;

        let amounts = log.token_amounts;
        let (tokens, token_amts): (Vec<_>, Vec<_>) = details.into_iter()
.enumerate().map(|(i, t)|
        {
            let token = db_tx.try_fetch_token_info(t)?;
            let decimals = token.decimals;
            Ok((token, amounts[i].to_scaled_rational(decimals)))
        }
        ).collect::<eyre::Result<Vec<_>>>()?.into_iter().unzip();



        Ok(NormalizedBurn {
            protocol: Protocol::CurveBasePool3,
            trace_index: info.trace_idx,
            pool: info.target_address,
            from: info.from_address,
            recipient: info.from_address,
            token: tokens,
            amount: token_amts,
        })

    }
);

action_impl!(
    Protocol::CurveBasePool3,
    crate::CurveBase3::remove_liquidity_imbalanceCall,
    Burn,
    [..RemoveLiquidityImbalance],
    logs: true,
    |
    info: CallInfo,
    log: CurveBasePool3Remove_liquidity_imbalanceCallLogs,
    db_tx: &DB
    |{
        let log = log.remove_liquidity_imbalance_field?;

        let details = db_tx.get_protocol_details(info.target_address)?;

        let amounts = log.token_amounts;
        let (tokens, token_amts): (Vec<_>, Vec<_>) = details.into_iter()
.enumerate().map(|(i, t)|
        {
            let token = db_tx.try_fetch_token_info(t)?;
            let decimals = token.decimals;
            Ok((token, amounts[i].to_scaled_rational(decimals)))
        }
        ).collect::<eyre::Result<Vec<_>>>()?.into_iter().unzip();

        Ok(NormalizedBurn {
            protocol: Protocol::CurveBasePool3,
            trace_index: info.trace_idx,
            pool: info.target_address,
            from: info.from_address,
            recipient: info.from_address,
            token: tokens,
            amount: token_amts,
        })

    }
);

action_impl!(
    Protocol::CurveBasePool3,
    crate::CurveBase3::remove_liquidity_one_coinCall,
    Burn,
    [..RemoveLiquidityOne],
    logs: true,
    call_data: true,
    |
    info: CallInfo,
    call_data: remove_liquidity_one_coinCall,
    log: CurveBasePool3Remove_liquidity_one_coinCallLogs,
    db_tx: &DB
    |{
        let log = log.remove_liquidity_one_field;

        let details = db_tx.get_protocol_details(info.target_address)?;

        let token = match call_data.i {
            0 => details.token0,
            1 => details.token1,
            2 => details.token2.ok_or(eyre::eyre!("Expected token2 for burn token, found None"))?,
            3 => details.token3.ok_or(eyre::eyre!("Expected token3 for burn token, found None"))?,
            4 => details.token4.ok_or(eyre::eyre!("Expected token4 for burn token, found None"))?,
            _ => unreachable!()
        };

        let token_info = db_tx.try_fetch_token_info(token)?;
        let amt = log.token_amount.to_scaled_rational(token_info.decimals);


        Ok(NormalizedBurn {
            protocol: Protocol::CurveBasePool3,
            trace_index: info.trace_idx,
            pool: info.target_address,
            from: info.from_address,
            recipient: info.from_address,
            token: vec![token_info],
            amount: vec![amt],
        })

    }
);
