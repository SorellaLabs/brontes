use std::{pin::Pin, task::Poll};

use alloy_primitives::{Address, Bytes, FixedBytes};
use alloy_providers::provider::Provider;
use alloy_rpc_types::TransactionRequest;
use alloy_sol_macro::sol;
use alloy_sol_types::SolCall;
use alloy_transport::TransportResult;
use alloy_transport_http::Http;
use brontes_database::database::Database;
use brontes_types::cache_decimals;
use futures::{future::join, join, stream::FuturesUnordered, Future, StreamExt};
use tracing::{debug, info, warn};

sol!(
    function decimals() public view returns (uint8);
);

type DecimalQuery<'a> =
    Pin<Box<dyn Future<Output = (Address, TransportResult<Bytes>)> + Send + 'a>>;

pub struct MissingDecimals<'db> {
    provider:         &'db Provider<Http<reqwest::Client>>,
    pending_decimals: FuturesUnordered<DecimalQuery<'db>>,
    db_future:        FuturesUnordered<Pin<Box<dyn Future<Output = ()> + Send + 'db>>>,
    database:         &'db Database,
}

impl<'db> MissingDecimals<'db> {
    pub fn new(
        provider: &'db Provider<Http<reqwest::Client>>,
        db: &'db Database,
        missing: Vec<Address>,
    ) -> Self {
        let mut this = Self {
            provider,
            pending_decimals: FuturesUnordered::default(),
            db_future: FuturesUnordered::default(),
            database: db,
        };
        this.missing_decimals(missing);

        this
    }

    fn missing_decimals(&mut self, addrs: Vec<Address>) {
        addrs.into_iter().for_each(|addr| {
            let call = decimalsCall::new(()).abi_encode();
            let mut tx_req = TransactionRequest::default().to(addr).input(call.into());

            self.pending_decimals
                .push(Box::pin(join(async move { addr }, self.provider.call(tx_req, None))));
        });
    }

    fn on_query_res(&mut self, addr: Address, res: TransportResult<Bytes>) {
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

impl Future for MissingDecimals<'_> {
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
