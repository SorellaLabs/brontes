use std::{pin::Pin, sync::Arc, task::Poll};

use alloy_primitives::{Address, Bytes};
use alloy_sol_macro::sol;
use alloy_sol_types::SolCall;
use brontes_database::clickhouse::Clickhouse;
use brontes_types::cache_decimals;
use futures::{future::join, join, stream::FuturesUnordered, Future, StreamExt};
use reth_provider::ProviderError;
use reth_rpc_types::{CallInput, CallRequest};
use tracing::{debug, info};

use crate::decoding::TracingProvider;

sol!(
    function decimals() public view returns (uint8);
);

type DecimalQuery<'a> =
    Pin<Box<dyn Future<Output = (Address, Result<Bytes, ProviderError>)> + Send + 'a>>;

pub struct MissingDecimals<'db, T: TracingProvider + 'db> {
    provider:         &'db Arc<T>,
    pending_decimals: FuturesUnordered<DecimalQuery<'db>>,
    db_future:        FuturesUnordered<Pin<Box<dyn Future<Output = ()> + Send + 'db>>>,
    _database:        &'db Clickhouse,
}

impl<'db, T: TracingProvider + 'static> MissingDecimals<'db, T> {
    pub fn new(provider: &'db Arc<T>, db: &'db Clickhouse, missing: Vec<Address>) -> Self {
        let mut this = Self {
            provider,
            pending_decimals: FuturesUnordered::default(),
            db_future: FuturesUnordered::default(),
            _database: db,
        };
        this.missing_decimals(missing);

        this
    }

    fn missing_decimals(&mut self, addrs: Vec<Address>) {
        addrs.into_iter().for_each(|addr| {
            let call = decimalsCall::new(()).abi_encode();
            // let tx_req = TransactionRequest::default().to(addr).input(call.into());
            let mut tx_req = CallRequest::default();
            tx_req.to = Some(addr);
            tx_req.input = CallInput::new(call.into());

            self.pending_decimals.push(Box::pin(join(
                async move { addr },
                self.provider.eth_call(tx_req, None, None, None),
            )));
        });
    }

    fn on_query_res(&mut self, addr: Address, res: Result<Bytes, ProviderError>) {
        if let Ok(res) = res {
            let Some(dec) = decimalsCall::abi_decode_returns(&res, false).ok() else { return };
            let dec = dec._0;
            info!(?dec, ?addr, "got new decimal");
            cache_decimals(**addr, dec);
            self.db_future.push(Box::pin(async {}));
        } else {
            debug!(?addr, "Token request failed for token");
        }
    }
}

impl<T: TracingProvider> Future for MissingDecimals<'_, T> {
    type Output = ();

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        while let Poll::Ready(Some(res)) = self.pending_decimals.poll_next_unpin(cx) {
            self.on_query_res(res.0, res.1);
        }

        while let Poll::Ready(Some(_)) = self.db_future.poll_next_unpin(cx) {}

        if self.pending_decimals.is_empty() && self.db_future.is_empty() {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}
