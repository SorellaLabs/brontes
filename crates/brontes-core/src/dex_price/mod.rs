use std::{collections::HashMap, pin::Pin, task::Poll};

use alloy_primitives::{Address, Bytes, FixedBytes};
use alloy_providers::provider::Provider;
use alloy_rpc_types::{state::AccountOverride, TransactionRequest};
use alloy_sol_macro::sol;
use alloy_sol_types::SolCall;
use alloy_transport::TransportResult;
use alloy_transport_http::Http;
use brontes_database::{database::Database, Pair};
use brontes_types::cache_decimals;
use futures::{future::join, join, stream::FuturesUnordered, Future, StreamExt};
use malachite::Rational;
use once_cell::sync::Lazy;
use phf::phf_map;
use reth_rpc_types::trace::parity::StateDiff;
use tokio::sync::futures;

use crate::TracingProvider;

pub mod uniswap_v2;
pub mod uniswap_v3;

pub trait DexPrice: Clone + Send + Sync + Unpin + 'static {
    fn get_price<T: TracingProvider>(
        &self,
        provider: Arc<T>,
        block: u64,
        address: Address,
        zto: bool,
        state_diff: StateDiff,
    ) -> Pin<Box<dyn Future<Output = (Rational, Rational)> + Send + Sync>>;
}

// we will have a static map for (token0, token1) => Vec<address, exchange type>
// this will then process using async, grab the reserves and process the price.
// and return that with tvl. with this we can calculate weighted price
pub struct DexPricing<T: TracingProvider> {
    provider: Arc<T>,
    futures: FuturesUnordered<
        Pin<Box<dyn Future<Output = (usize, HashMap<Pair, Rational>)> + Send + Sync>>,
    >,
    res:      HashMap<usize, HashMap<Pair, Rational>>,
}

impl<T: TracingProvider> DexPricing<T> {
    pub fn new(
        provider: Arc<T>,
        block: u64,
        pools_tokens_type: Vec<TransactionPoolSwappedTokens>,
    ) -> Self {
        let mut this =
            Self { provider, futures: FuturesUnordered::default(), res: HashMap::default() };

        pools_tokens_type.into_iter().for_each(|transaction| {
            this.futures.push(Box::pin(async {
                let mut result = HashMap::new();

                for pair in transaction.pairs {
                    let Some(dex) = DEX_PRICE_MAP.get(&pair) else {
                        continue;
                    };

                    let price_tvl: Vec<(Rational, Rational)> =
                        join_all(pair.into_iter().map(|(zto, addr, dex)| {
                            dex.get_price(self.provider, block, zto, addr, transaction.state_diff)
                        }))
                        .await;

                    let weights = price_tvl.iter().map(|(_, tvl)| tvl).sum::<Rational>();
                    let weighted_price = price_tvl
                        .into_iter()
                        .map(|(price, tvl)| price * tvl)
                        .sum::<Rational>()
                        / weights;

                    result.insert(key, weighted_price);
                }

                (transaction.tx_idx, result)
            }));
        });

        this
    }
}

impl<T: TracingProvider> Future for DexPricing<T> {
    type Output = HashMap<usize, HashMap<Pair, Rational>>;

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        while let Poll::Ready(Some((k, v))) = self.futures.poll_next_unpin(cx) {
            self.res.insert(k, v);
        }

        if self.futures.is_empty() {
            return Poll::Ready(self.res.drain().collect())
        } else {
            Poll::Pending
        }
    }
}

async fn make_call_request<C: SolCall, T: TracingProvider>(
    call: C,
    provider: Arc<T>,
    state: Option<StateOverride>,
    to: Address,
    block: u64,
) -> C::Return {
    let encoded = call.abi_encode();
    let req =
        CallRequest { to: Some(to), input: CallInput::new(encoded.into()), ..Default::default() };

    let res = provider
        .eth_call(req, Some(block), state, None)
        .await
        .unwrap();
    C::abi_decode_returns(&res, false).unwrap()
}

fn into_state_overrides(state_diff: StateDiff) -> AccountOverride {
    state_diff
        .0
        .into_iter()
        .map(|(k, v)| {
            let overrides = AccountOverride {
                nonce:      None,
                code:       None,
                state:      Some(
                    v.storage
                        .into_iter()
                        .filter_map(|(k, v)| {
                            Some((
                                k,
                                match v {
                                    Delta::Unchanged => return None,
                                    Delta::Added(a) => a,
                                    Delta::Removed(r) => return None,
                                    Delta::Changed(t) => t.to,
                                },
                            ))
                        })
                        .collect(),
                ),
                state_diff: None,
                balance:    None,
            };

            (k, overrides)
        })
        .collect::<HashMap<_, _>>()
}
