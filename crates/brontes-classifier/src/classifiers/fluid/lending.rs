use std::{str::FromStr, sync::Arc};

use alloy_dyn_abi::{DynSolType, DynSolValue};
use alloy_primitives::Address;
use alloy_sol_types::SolType;
use brontes_macros::{action_impl, discovery_impl};
use brontes_pricing::make_call_request;
use brontes_types::{
    constants::{FLUID_VAULT_FACTORY_ADDRESS, FLUID_VAULT_RESOLVER_ADDRESS},
    normalized_actions::{NormalizedBurn, NormalizedMint, NormalizedNewPool, NormalizedSwap},
    structured_trace::CallInfo,
    traits::TracingProvider,
    utils::ToScaledRational,
    Protocol,
};

use crate::FluidVaultResolver;
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
