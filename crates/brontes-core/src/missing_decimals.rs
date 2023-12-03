use std::task::Poll;

use alloy_primitives::{Address, Bytes};
use alloy_providers::provider::Provider;
use alloy_transport::TransportResult;
use alloy_transport_http::Http;
use brontes_database::database::Database;
use futures::{stream::FuturesUnordered, Future, StreamExt};

sol!(
    function decimals() public view returns (uint8);
);

type DecimalQuery<'a> = Pin<Box<dyn Future<Output = TransportResult<Bytes>> + Send + Sync + 'a>>;

pub struct MissingDecimals<'db> {
    provider:         Provider<Http<reqwest::Client>>,
    pending_decimals: FuturesUnordered<DecimalQuery<'a>>,
    db_future:        FuturesUnordered<Pin<Box<dyn Future<Output = ()>>>>,
    database:         &'db Database,
}

impl<'db> MissingDecimals<'db> {
    pub fn new(url: &String, db: &'db Database, missing: Vec<Address>) -> Self {
        let mut this = Self {
            provider:         Provider::new(url).unwrap(),
            pending_decimals: FuturesUnordered::default(),
            db_future:        FuturesUnordered::default(),
            database:         db,
        };
        this.missing_decimals(missing);

        this
    }

    fn missing_decimals(&mut self, addrs: Vec<Address>) {
        addrs.into_iter().for_each(|addr| {
            let call = decimalsCall::new(()).abi_encode();
            let mut tx_req = TransactionRequest::default()
                .to(Address(FixedBytes(addr.clone())))
                .input(call);

            self.pending_decimals
                .push(self.provider.call(tx_req, None).await);
        });
    }

    fn on_query_res(&mut self, res: TransportResult<Bytes>) {
        if let Ok(res) = res {
            let Some(dec) = decimalsCall::abi_decode_returns(&res, true).ok() else {
                warn!("failed to decode decimal call");
                return
            };
            let dec = dec._0;
            cache_decimals(addr, dec);
            self.db_future.push(Box::pin(async {}));
        } else {
            warn!("Token request failed for token");
        }
    }
}

impl Future for MissingDecimals {
    type Output = ();

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        while let Poll::Ready(Some(res)) = self.pending_decimals.poll_next_unpin(cx) {
            self.on_query_res(res);
        }

        while let Poll::Ready(Some(_)) = self.db_future.poll_next_unpin(cx) {}

        if self.pending_decimals.is_empty() && self.db_future.is_empty() {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}
