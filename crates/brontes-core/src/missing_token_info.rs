use std::sync::Arc;

use alloy_primitives::Address;
use alloy_sol_macro::sol;
use alloy_sol_types::SolCall;
use brontes_database::libmdbx::LibmdbxWriter;
use brontes_types::make_call_request;
use futures::{join, stream::FuturesUnordered, StreamExt};
use tracing::error;

use crate::decoding::TracingProvider;

sol!(
    function decimals() public view returns (uint8);
    function symbol() public view returns (string);
);

pub async fn load_missing_token_info<T: TracingProvider, W: LibmdbxWriter>(
    provider: &Arc<T>,
    db: &W,
    block: u64,
    missing_address: Address,
) {
    let data = query_missing_data(provider, block, missing_address).await;
    on_decimal_query_resolution(db, data);
}

pub async fn load_missing_token_infos<T: TracingProvider, W: LibmdbxWriter>(
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
        on_decimal_query_resolution(db, res);
    }
}

async fn query_missing_data<T: TracingProvider>(
    provider: &Arc<T>,
    block: u64,
    missing_address: Address,
) -> eyre::Result<(Address, u8, String)> {
    let (decimals, symbol) = join!(
        make_call_request(decimalsCall::new(()), provider, missing_address, Some(block)),
        make_call_request(symbolCall::new(()), provider, missing_address, Some(block))
    );
    decimals.map(|d| (missing_address, d._0, symbol.map(|s| s._0).unwrap_or_default()))
}

fn on_decimal_query_resolution<W: LibmdbxWriter>(
    database: &W,
    result: eyre::Result<(Address, u8, String)>,
) {
    match result {
        Ok((address, decimals, symbol)) => {
            if let Err(e) = database.write_token_info(address, decimals, symbol) {
                error!(error= %e, "failed to write token info into database");
            }
        }
        Err(e) => {
            error!(error= %e, "token info request failed");
        }
    }
}
