use std::{collections::HashMap, future::Future, pin::Pin, sync::Arc, task::Poll};

use ::futures::{future::join_all, stream::FuturesUnordered};
use alloy_primitives::Address;
use alloy_rpc_types::state::AccountOverride;
use alloy_sol_types::SolCall;
use brontes_database::{
    graph::{PriceGraph, TrackableGraph},
    DexQuote, Pair,
};
use brontes_types::extra_processing::TransactionPoolSwappedTokens;
use itertools::Itertools;
use malachite::Rational;
use phf::phf_map;
use reth_primitives::revm_primitives::HashSet;
use reth_rpc_types::{
    state::StateOverride,
    trace::parity::{Delta, StateDiff},
    CallInput, CallRequest,
};
use tracing::error;

use crate::decoding::TracingProvider;

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

static DEX_TOKEN_MAP: phf::Map<Pair, ()> = phf_map!();

// we have a table in libmdbx that maps (token0, token1) => Vec<address,
// exchange type> this will then process using async, grab the reserves and
// process the price. and return that with tvl. with this we can calculate
// weighted price
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

    fn back_fill(&mut self) {
        let map = self.res.drain().collect::<Vec<_>>();
        let mut res: HashMap<Pair, Vec<(usize, Rational)>> = HashMap::new();
        for (block, pairs) in map {
            pairs
                .into_iter()
                .for_each(|(pair, price)| res.entry(pair).or_default().push((block, price)))
        }

        // extend out to fill in non state change blocks
        let res = res
            .into_iter()
            .map(|(k, mut v)| {
                let mut res = HashMap::new();
                for i in 0..v.len() {
                    let (cur_idx, p) = v[i];
                    if let Some((next_idx, _)) = v.get(i + 1) {
                        for i in cur_idx..*next_idx {
                            res.insert(i, p.clone());
                        }
                    }
                }

                (k, DexQuote(res))
            })
            .collect();

        let map = QuotesMap::wrap(res);
        let (disjointed, graph) = PriceGraph::from_quotes_disjoint(map);
        if disjointed.is_empty() {
            // TODO: quick return here
        }

        // make graph of all known tokens
        let dummy_graph = TrackableGraph::from_hash_map(
            DEX_TOKEN_MAP
                .into_iter()
                .map(|(k, _)| ((k.0, k.1), ()))
                .collect(),
        );

        let combos = disjointed.into_iter().combinations(2).collect::<Vec<_>>();
        let mut needed_pairs = HashSet::new();

        for mut combo in combos {
            let start = combo.remove(0);
            let end = combo.remove(0);

            if let Some(path) = dummy_graph.get_path(start, end) {
                needed_pairs.extend(
                    path.into_iter()
                        .tuple_windows()
                        .map(|(t0, t1)| Pair(t0, t1)),
                )
            } else {
                error!(
                    ?start,
                    ?end,
                    "no path between tokens over all known dexes... if you see this its prob \
                     worth a screenshot bc of how rare this would be"
                );
            }
        }
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
            self.back_fill();
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
