use std::sync::Arc;

use alloy_primitives::{Address, Bytes};
use alloy_sol_macro::sol;
use alloy_sol_types::SolCall;
use brontes_database::libmdbx::LibmdbxWriter;
use futures::{future::join, stream::FuturesUnordered, StreamExt};
use reth_provider::ProviderError;
use reth_rpc_types::{CallInput, CallRequest};
use tracing::{debug, error};

use crate::decoding::TracingProvider;

sol!(
    function decimals() public view returns (uint8);
);
pub async fn load_missing_decimal<T: TracingProvider, W: LibmdbxWriter>(
    provider: Arc<T>,
    db: &W,
    block: u64,
    missing_address: Address,
) {
    let call = decimalsCall::new(()).abi_encode();
    let mut tx_req = CallRequest::default();
    tx_req.to = Some(missing_address);
    tx_req.input = CallInput::new(call.into());

    let p = provider.clone();
    let res = p.eth_call(tx_req, Some(block.into()), None, None).await;

    on_decimal_query_resolution(db, missing_address, res);
}

pub async fn load_missing_decimals<T: TracingProvider, W: LibmdbxWriter>(
    provider: Arc<T>,
    db: &W,
    block: u64,
    missing: Vec<Address>,
) {
    let mut pending_decimals = FuturesUnordered::new();
    missing.into_iter().for_each(|addr| {
        let call = decimalsCall::new(()).abi_encode();
        let mut tx_req = CallRequest::default();
        tx_req.to = Some(addr);
        tx_req.input = CallInput::new(call.into());

        let p = provider.clone();
        pending_decimals.push(Box::pin(join(async move { addr }, async move {
            p.eth_call(tx_req, Some(block.into()), None, None).await
        })));
    });

    while let Some((address, bytes)) = pending_decimals.next().await {
        on_decimal_query_resolution(db, address, bytes);
    }
}

fn on_decimal_query_resolution<W: LibmdbxWriter>(
    database: &W,
    addr: Address,
    res: Result<Bytes, ProviderError>,
) {
    if let Ok(res) = res {
        let Some(dec) = decimalsCall::abi_decode_returns(&res, false).ok() else { return };
        let dec = dec._0;
        debug!(?dec, ?addr, "got new decimal");
        if let Err(e) = database.write_token_decimals(addr, dec) {
            error!(?e, "failed to insert missing decimals into libmdbx");
        }
    } else {
        // this is a debug as its pretty common to come across tokens
        // without a decimals fn
        debug!(?addr, "Token request failed for token");
    }
}
