use brontes_macros::action_impl;
use brontes_pricing::Protocol;
use brontes_types::{
    normalized_actions::NormalizedBurn, structured_trace::CallInfo, ToScaledRational,
};

action_impl!(
    Protocol::CurveBasePool4,
    crate::CurveBase4::remove_liquidityCall,
    Burn,
    [..RemoveLiquidity],
    logs: true,
    |
    info: CallInfo,
    log: CurveBasePool4remove_liquidityCallLogs,
    db_tx: &DB
    |{
        let log = log.RemoveLiquidity_field;

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
            protocol: Protocol::CurveBasePool4,
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
    Protocol::CurveBasePool4,
    crate::CurveBase4::remove_liquidity_imbalanceCall,
    Burn,
    [..RemoveLiquidityImbalance],
    logs: true,
    |
    info: CallInfo,
    log: CurveBasePool4remove_liquidity_imbalanceCallLogs,
    db_tx: &DB
    |{
        let log = log.RemoveLiquidityImbalance_field;

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
            protocol: Protocol::CurveBasePool4,
            trace_index: info.trace_idx,
            pool: info.target_address,
            from: info.from_address,
            recipient: info.from_address,
            token: tokens,
            amount: token_amts,
        })

    }
);

// no remove_liquidity_one_coinCall for 4 token base pools
