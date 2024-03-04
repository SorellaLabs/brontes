use brontes_macros::action_impl;
use brontes_pricing::Protocol;
use brontes_types::{
    normalized_actions::NormalizedBurn, structured_trace::CallInfo, ToScaledRational,
};

action_impl!(
    Protocol::CurveBasePool2,
    crate::CurveLido2::remove_liquidity_one_coinCall,
    Burn,
    [..RemoveLiquidityOne],
    logs: true,
    call_data: true,
    |
    info: CallInfo,
    call_data: remove_liquidity_one_coinCall,
    log: CurveBasePool2Remove_liquidity_one_coinCallLogs,
    db_tx: &DB
    |{
        let log = log.remove_liquidity_one_field?;

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
            protocol: Protocol::CurveBasePool2,
            trace_index: info.trace_idx,
            pool: info.target_address,
            from: info.from_address,
            recipient: info.from_address,
            token: vec![token_info],
            amount: vec![amt],
        })

    }
);
