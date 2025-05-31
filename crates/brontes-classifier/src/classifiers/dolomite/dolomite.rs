use brontes_macros::action_impl;
use brontes_types::{
    normalized_actions::{NormalizedLiquidation, NormalizedNewPool},
    structured_trace::CallInfo,
    utils::ToScaledRational,
    Protocol,
};

action_impl!(
    Protocol::Dolomite,
    crate::DolomiteAdmin::ownerAddMarketCall,
    NewPool,
    [..LogAddMarket],
    logs: true,
    |info: CallInfo, log_data: DolomiteOwnerAddMarketCallLogs, db:&DB| {
        let log_data=log_data.log_add_market_field?;
        let token=log_data.token;

        let pool_address=info.target_address;
        let protocol_details=db.get_protocol_details(pool_address);
        match protocol_details {
            Ok(protocol_detail) => {
                let mut tokens=protocol_detail.get_tokens();
                tokens.push(token);
                Ok(NormalizedNewPool{
                    trace_index: info.trace_idx,
                    protocol: Protocol::Dolomite,
                    pool_address,
                    tokens
                })
            },
            _ => {
                Ok(NormalizedNewPool{
                    trace_index: info.trace_idx,
                    protocol: Protocol::Dolomite,
                    pool_address,
                    tokens: vec![token]
                })
            }
        }
    }
);

action_impl!(
    Protocol::Dolomite,
    crate::DolomiteLiquidator::operateCall,
    Liquidation,
    [..LogLiquidate],
    call_data: true,
    logs:true,
    |
    info: CallInfo,
    _call_data: operateCall,
    logs: DolomiteOperateCallLogs,
    db: &DB | {

        let log_data=logs.log_liquidate_field?;

        let liquidator=log_data.solidAccountOwner;
        let debtor=log_data.liquidAccountOwner;

        let target_address=info.target_address;
        let protocol_details=db.get_protocol_details(target_address)?;

        let tokens = protocol_details.get_tokens();


        
        let held_market_idx = log_data.heldMarket.to::<usize>();
        let owed_market_idx = log_data.owedMarket.to::<usize>();

        // collateral market
        let collateral_token = tokens[held_market_idx];
        let debt_token = tokens[owed_market_idx];
        let collateral_info = db.try_fetch_token_info(collateral_token)?;
        // debt market
        let debt_info = db.try_fetch_token_info(debt_token)?;
        let covered_debt = log_data.solidHeldUpdate.deltaWei.value.to_scaled_rational(debt_info.decimals);
        let liquidated_collateral = log_data.solidOwedUpdate.deltaWei.value.to_scaled_rational(collateral_info.decimals);

        return Ok(NormalizedLiquidation {
            protocol: Protocol::Dolomite,
            trace_index: info.trace_idx,
            pool: info.from_address,
            liquidator,
            debtor,
            collateral_asset: collateral_info,
            debt_asset: debt_info,
            covered_debt,
            liquidated_collateral,
            msg_value: info.msg_value,
        })
    }
);
