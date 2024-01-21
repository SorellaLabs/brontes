use std::{pin::Pin, sync::Arc, task::Poll};

use alloy_primitives::{Address, Bytes};
use alloy_sol_macro::sol;
use alloy_sol_types::SolCall;
use brontes_database::libmdbx::{
    tables::TokenDecimals, types::token_decimals::TokenDecimalsData, Libmdbx,
};
use futures::{future::join, stream::FuturesUnordered, Future, StreamExt};
use reth_provider::ProviderError;
use reth_rpc_types::{CallInput, CallRequest};
use tracing::{debug, error};

use crate::decoding::TracingProvider;

sol!(
    function decimals() public view returns (uint8);
);

type DecimalQuery = Pin<Box<dyn Future<Output = (Address, Result<Bytes, ProviderError>)> + Send>>;

pub struct MissingDecimals<'db, T: TracingProvider + 'db> {
    provider:         Arc<T>,
    pending_decimals: FuturesUnordered<DecimalQuery>,
    database:         &'db Libmdbx,
}

impl<'db, T: TracingProvider + 'static> MissingDecimals<'db, T> {
    pub fn new(provider: Arc<T>, db: &'db Libmdbx, block: u64, missing: Vec<Address>) -> Self {
        let mut this =
            Self { provider, pending_decimals: FuturesUnordered::default(), database: db };
        this.missing_decimals(block, missing);

        this
    }

    fn missing_decimals(&mut self, block: u64, addrs: Vec<Address>) {
        addrs.into_iter().for_each(|addr| {
            let call = decimalsCall::new(()).abi_encode();
            let mut tx_req = CallRequest::default();
            tx_req.to = Some(addr);
            tx_req.input = CallInput::new(call.into());

            let p = self.provider.clone();
            self.pending_decimals
                .push(Box::pin(join(async move { addr }, async move {
                    p.eth_call(tx_req, Some(block.into()), None, None).await
                })));
        });
    }

    fn on_query_res(&mut self, addr: Address, res: Result<Bytes, ProviderError>) {
        if let Ok(res) = res {
            let Some(dec) = decimalsCall::abi_decode_returns(&res, false).ok() else { return };
            let dec = dec._0;
            debug!(?dec, ?addr, "got new decimal");
            if let Err(e) = self.insert_decimals(addr, dec) {
                error!(?e, "failed to insert missing decimals into libmdbx");
            }
        } else {
            // this is a debug as its pretty common to come across tokens
            // without a decimals fn
            debug!(?addr, "Token request failed for token");
        }
    }

    fn insert_decimals(&self, address: Address, decimals: u8) -> eyre::Result<()> {
        self.database
            .write_table::<TokenDecimals, TokenDecimalsData>(&vec![TokenDecimalsData {
                address,
                decimals,
            }])
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

        if self.pending_decimals.is_empty() {
            Poll::Ready(())
        } else {
            Poll::Pending
        }
    }
}
