use std::sync::Arc;

use alloy_primitives::Address;
use alloy_sol_macro::sol;
use alloy_sol_types::SolCall;
use brontes_database::libmdbx::DBWriter;
use brontes_types::make_call_request;
use futures::{join, stream::FuturesUnordered, StreamExt};
use tracing::error;

use crate::decoding::TracingProvider;

sol!(
    interface normal {
        function decimals() public view returns (uint8);
        function symbol() public view returns (string);
    }
);
sol!(
    interface autistic {
        function symbol() public view returns (bytes32);
    }
);

pub async fn load_missing_token_info<T: TracingProvider, W: DBWriter>(
    provider: &Arc<T>,
    db: &W,
    block: u64,
    missing_address: Address,
) {
    let data = query_missing_data(provider, block, missing_address).await;
    on_decimal_query_resolution(db, data).await;
}

pub async fn load_missing_token_infos<T: TracingProvider, W: DBWriter>(
    provider: &Arc<T>,
    db: &W,
    block: u64,
    missing: Vec<Address>,
) {
    let mut pending_decimals = FuturesUnordered::new();
    missing
        .into_iter()
        .for_each(|addr| pending_decimals.push(query_missing_data(provider, block, addr)));

    while let Some(res) = pending_decimals.next().await {
        on_decimal_query_resolution(db, res).await;
    }
}

async fn query_missing_data<T: TracingProvider>(
    provider: &Arc<T>,
    block: u64,
    missing_address: Address,
) -> eyre::Result<(Address, u8, String)> {
    let (decimals, symbol, symbol_autistic) = join!(
        make_call_request(normal::decimalsCall::new(()), provider, missing_address, Some(block)),
        make_call_request(normal::symbolCall::new(()), provider, missing_address, Some(block)),
        make_call_request(autistic::symbolCall::new(()), provider, missing_address, Some(block))
    );

    Ok(decimals.map(|d| d._0).unwrap_or_default()).map(|d| {
        (
            missing_address,
            d,
            symbol.map(|s| s._0).unwrap_or_else(|_| {
                symbol_autistic
                    .map(|s| String::from_utf8((s._0).to_vec()).unwrap_or_default())
                    .unwrap_or_default()
            }),
        )
    })
}

async fn on_decimal_query_resolution<W: DBWriter>(
    database: &W,
    result: eyre::Result<(Address, u8, String)>,
) {
    match result {
        Ok((address, decimals, symbol)) => {
            if let Err(e) = database.write_token_info(address, decimals, symbol).await {
                error!(error= %e, "failed to write token info into database");
            }
        }
        Err(e) => {
            error!(error= %e, "token info request failed");
        }
    }
}
