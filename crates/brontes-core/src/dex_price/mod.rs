use std::{collections::HashMap, pin::Pin, task::Poll};

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
use malachite::Rational;
use once_cell::sync::Lazy;
use phf::phf_map;
use reth_rpc_types::trace::parity::StateDiff;
use tokio::sync::futures;

pub mod uniswap_v2;
pub mod uniswap_v3;

static DEX_PRICE_MAP: phf::Map<[u8; 40], &[(bool, Address, Lazy<Box<dyn DexPrice>>)]> =
    phf::phf_map!();

pub struct TransactionPoolSwappedTokens {
    tx_idx:     usize,
    pairs:      Vec<(Address, Address)>,
    state_diff: StateDiff,
}

pub trait DexPrice: Clone + Send + Sync + Unpin + 'static {
    fn get_price(
        &self,
        provider: &Provider<Http<reqwest::Client>>,
        address: Address,
        zto: bool,
        state_diff: StateDiff,
    ) -> Pin<Box<dyn Future<Output = (Rational, Rational)> + Send + Sync>>;
}

// we will have a static map for (token0, token1) => Vec<address, exchange type>
// this will then process using async, grab the reserves and process the price.
// and return that with tvl. with this we can calculate weighted price
pub struct DexPricing<'p> {
    provider: &'p Provider<Http<reqwest::Client>>,
    futures: FuturesUnordered<
        Pin<Box<dyn Future<Output = (usize, HashMap<[u8; 40], Rational>)> + Send + Sync>>,
    >,
    res:      HashMap<usize, HashMap<[u8; 40], Rational>>,
}

impl<'p> DexPricing<'p> {
    pub fn new(
        provider: &'p Provider<Http<reqwest::Client>>,
        pools_tokens_type: Vec<TransactionPoolSwappedTokens>,
    ) -> Self {
        let mut this =
            Self { provider, futures: FuturesUnordered::default(), res: HashMap::default() };

        pools_tokens_type.into_iter().for_each(|transaction| {
            this.futures.push(Box::pin(async {
                let mut result = HashMap::new();

                for (t0, t1) in transaction.pairs {
                    let key = combine_slices(t0.0 .0, t1.0 .0);
                    let Some(pairs) = DEX_PRICE_MAP.get(&key) else {
                        continue;
                    };

                    let price_tvl: Vec<(Rational, Rational)> =
                        join_all(pair.into_iter().map(|(zto, addr, dex)| {
                            dex.get_price(self.provider, zto, addr, transaction.state_diff)
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

impl Future for DexPricing<'_> {
    type Output = HashMap<usize, HashMap<[u8; 40], Rational>>;

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

fn combine_slices(slice1: [u8; 20], slice2: [u8; 20]) -> [u8; 40] {
    let mut combined = [0u8; 40];

    combined[..20].copy_from_slice(&slice1);
    combined[20..].copy_from_slice(&slice2);

    combined
}
