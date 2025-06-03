use std::sync::Arc;

use alloy_primitives::{Address, Uint, U256};
use brontes_macros::{action_impl, discovery_impl};
use brontes_pricing::make_call_request;
use brontes_types::{
    constants::{FLUID_DEX_RESOLVER_ADDRESS, FLUID_VAULT_RESOLVER_ADDRESS},
    normalized_actions::{NormalizedLiquidation, NormalizedNewPool},
    structured_trace::CallInfo,
    traits::TracingProvider,
    Protocol, ToScaledRational,
};
// Add trait imports for U256 arithmetic operations
use std::ops::{Mul, Div};

use crate::{FluidDexResolver, FluidVaultResolver};
discovery_impl!(
    FluidLendingDiscovery,
    crate::FluidVaultFactory::deployVaultCall,
    0x324c5Dc1fC42c7a4D43d92df1eBA58a54d13Bf2d,
    |deployed_address: Address, trace_index: u64, _: deployVaultCall, tracer: Arc<T>| async move {
        parse_market_pool(
            Protocol::FluidLending,
            deployed_address,
            FLUID_VAULT_RESOLVER_ADDRESS,
            trace_index,
            tracer,
        )
        .await
    }
);

action_impl!(
    Protocol::FluidLending,
    crate::FluidVault::liquidate_0Call,
    Liquidation,
    [LogLiquidate],
    logs:true,
    |info: CallInfo, log_data: FluidLendingLiquidate_0CallLogs,db_tx: &DB| {
        let pool_address = info.target_address;
        let logs=log_data.log_liquidate_field?;
        let protocol_details = db_tx.get_protocol_details(pool_address)?;

        let supply_asset = protocol_details.token0;
        let borrow_asset = protocol_details.token2.ok_or_else(|| eyre::eyre!("Token3 does not exist"))?;

        let supply_info = db_tx.try_fetch_token_info(supply_asset)?;
        let borrow_info = db_tx.try_fetch_token_info(borrow_asset)?;

        let liquidator = logs.liquidator_;

        let actual_col_amt = logs.colAmt_.to_scaled_rational(supply_info.decimals);
        let actual_debt_amt = logs.debtAmt_.to_scaled_rational(borrow_info.decimals);

        // as it liquidates the entire position, we don't know the debtor address
        Ok(NormalizedLiquidation {
            pool: pool_address,
            liquidator,
            debtor: Address::ZERO,
            collateral_asset: supply_info,
            debt_asset: borrow_info,
            covered_debt: actual_debt_amt,
            msg_value: info.msg_value,
            liquidated_collateral: actual_col_amt,
            trace_index: info.trace_idx,
            protocol: Protocol::FluidLending,
        })
    }
);

action_impl!(
    Protocol::FluidLending,
    crate::FluidVault::liquidate_1Call,
    Liquidation,
    [LogLiquidate],
    logs: true,
    |info: CallInfo, log_data: FluidLendingLiquidate_1CallLogs, db_tx: &DB| {

        let pool_address = info.target_address;
        
        // Handle potential errors in sync part and return early with error future if needed
        let logs = log_data.log_liquidate_field?;
        let protocol_details = db_tx.get_protocol_details(pool_address)?;

        let supply_asset_token0 = protocol_details.token0;
        let supply_asset_token1 = protocol_details.token1;
        let borrow_asset_token0 = protocol_details.token2.ok_or_else(|| eyre::eyre!("Token3 does not exist"))?;
        let borrow_asset_token1 = protocol_details.token3.ok_or_else(|| eyre::eyre!("Token4 does not exist"))?;

        let is_smart_col = supply_asset_token1 != Address::ZERO;
        let is_smart_borrow = borrow_asset_token1 != Address::ZERO;

        let supply_info_token0 = db_tx.try_fetch_token_info(supply_asset_token0)?;
        let borrow_info_token0 = db_tx.try_fetch_token_info(borrow_asset_token0)?;
        
        let liquidator = logs.liquidator_;

        let exchange_rates = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(
                query_fluid_dex_state(&tracer, &pool_address, FLUID_DEX_RESOLVER_ADDRESS, is_smart_col, is_smart_borrow)
            )
        });

        let actual_col_amt = logs.colAmt_.mul(exchange_rates[0]).div(U256::from(10u128).pow(U256::from(18u128))).to_scaled_rational(supply_info_token0.decimals);
        let actual_debt_amt = logs.debtAmt_.mul(exchange_rates[1]).div(U256::from(10u128).pow(U256::from(18u128))).to_scaled_rational(borrow_info_token0.decimals);

        Ok(NormalizedLiquidation {
            pool: pool_address,
            liquidator,
            debtor: Address::ZERO,
            collateral_asset: supply_info_token0,
            debt_asset: borrow_info_token0,
            covered_debt: actual_debt_amt,
            msg_value: info.msg_value,
            liquidated_collateral: actual_col_amt,
            trace_index: info.trace_idx,
            protocol: Protocol::FluidLending,
        })
    }
);

pub async fn query_fluid_dex_state<T: TracingProvider>(
    tracer: &Arc<T>,
    vault: &Address,
    dex_resolver: Address,
    is_smart_col: bool,
    is_smart_borrow: bool,
) -> Vec<Uint<256, 4>> {
    let mut result = vec![];
    if is_smart_col || is_smart_borrow {
        if let Ok(call_return) =
            make_call_request(FluidVaultResolver::getVaultEntireDataCall { vault_: *vault }, tracer, *vault, None).await
        {

            if is_smart_col {
                let smart_col_dex=call_return.vaultData_.constantVariables.supply;
                if let Ok(call_return) = make_call_request(
                    FluidDexResolver::getDexStateCall { dex_: smart_col_dex },
                    tracer,
                    dex_resolver,
                    None,
                )
                .await {
                    let state = call_return.state_;
                    result.push(state.token0PerSupplyShare);
                }else {
                    result.push(U256::from(10u128).pow(U256::from(18u128)));
                }
            }else {
                result.push(U256::from(10u128).pow(U256::from(18u128)));
            }
            if is_smart_borrow {
                let smart_borrow_dex=call_return.vaultData_.constantVariables.borrow;
                if let Ok(call_return) = make_call_request(
                    FluidDexResolver::getDexStateCall { dex_: smart_borrow_dex },
                    tracer,
                    dex_resolver,
                    None,
                )
                .await {
                    let state = call_return.state_;
                    result.push(state.token1PerBorrowShare);
                }else{
                    result.push(U256::from(10u128).pow(U256::from(18u128)));
                }
            }else {
                result.push(U256::from(10u128).pow(U256::from(18u128)));
            }
        }
    } else {
        result.push(U256::from(10u128).pow(U256::from(18u128)));
        result.push(U256::from(10u128).pow(U256::from(18u128)));
    }
    result
}

pub async fn query_fluid_lending_market_tokens<T: TracingProvider>(
    tracer: &Arc<T>,
    vault: &Address,
    vault_resolver: Address,
) -> Vec<Address> {
    let mut result = vec![];
    if let Ok(call_return) = make_call_request(
        FluidVaultResolver::getVaultEntireDataCall { vault_: *vault },
        tracer,
        vault_resolver,
        None,
    )
    .await
    {
        let vault_data = call_return.vaultData_;

        let supply_tokens = vault_data.constantVariables.supplyToken;
        let borrow_tokens = vault_data.constantVariables.borrowToken;
        result.push(supply_tokens.token0);
        result.push(supply_tokens.token1);
        result.push(borrow_tokens.token0);
        result.push(borrow_tokens.token1);
    }
    result
}

async fn parse_market_pool<T: TracingProvider>(
    protocol: Protocol,
    deployed_address: Address,
    vault_resolver: Address,
    trace_index: u64,
    tracer: Arc<T>,
) -> Vec<NormalizedNewPool> {
    let tokens =
        query_fluid_lending_market_tokens(&tracer, &deployed_address, vault_resolver).await;

    vec![NormalizedNewPool { trace_index, protocol, pool_address: deployed_address, tokens }]
}
