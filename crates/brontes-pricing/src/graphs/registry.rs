use std::collections::BTreeMap;

use alloy_primitives::Address;
use brontes_metrics::pricing::DexPricingMetrics;
use brontes_types::{pair::Pair, FastHashMap};
use itertools::Itertools;
use malachite::{
    num::{
        arithmetic::traits::Reciprocal,
        basic::traits::{One, Zero},
    },
    Rational,
};

use super::{subgraph::PairSubGraph, PoolState};
use crate::types::{PairWithFirstPoolHop, ProtocolState};

/// Manages subgraphs in the BrontesBatchPricer module, crucial for DEX pricing.
///
/// [`SubGraphRegistry`] handles dynamic management and maintenance of verified
/// subgraphs, representing various token pairs. It responds to changes in the
/// DEX, like new liquidity pools or updates in existing ones, by extending or
/// updating subgraphs accordingly.
///
/// The registry facilitates accurate price retrieval for specific token pairs
/// based on their subgraphs. It also addresses situations where a pool in a
/// subgraph is compromised, ensuring the integrity and accuracy of pricing
/// information.
///
/// Mainly functioning within the BrontesBatchPricer system, it plays a key role
/// in providing up-to-date and reliable pricing data in the decentralized
/// exchange context.
#[derive(Debug, Clone)]
pub struct SubGraphRegistry {
    /// all currently known sub-graphs
    sub_graphs:               FastHashMap<Pair, BTreeMap<Pair, PairSubGraph>>,
    /// the pending_subgrpahs that haven't been finalized yet.
    pending_finalized_graphs: FastHashMap<u64, PendingRegistry>,
    /// metrics
    metrics:                  Option<DexPricingMetrics>,
}

/// holder for subgraphs that aren't active yet to avoid race conditions
#[derive(Debug, Clone, Default)]
pub struct PendingRegistry {
    sub_graphs: FastHashMap<Pair, BTreeMap<Pair, PairSubGraph>>,
}

impl Drop for SubGraphRegistry {
    fn drop(&mut self) {
        let subgraphs_cnt = self.sub_graphs.values().map(|f| f.len()).sum::<usize>();

        tracing::debug!(
            target: "brontes::mem",
            pending_finalized_subs = self.pending_finalized_graphs.len(),
            subgraphs_len = subgraphs_cnt,
            "subgraph registry final"
        );
    }
}

impl SubGraphRegistry {
    pub fn new(metrics: Option<DexPricingMetrics>) -> Self {
        let sub_graphs = FastHashMap::default();
        Self { sub_graphs, pending_finalized_graphs: FastHashMap::default(), metrics }
    }

    // for all subgraphs that haven't been used in a given time period, will
    // remove them from and return each pool with the amount to decrement.
    pub fn prune_dead_subgraphs(&mut self, block: u64) -> FastHashMap<Address, u64> {
        let mut removals = FastHashMap::default();

        self.sub_graphs.retain(|p, vec| {
            vec.retain(|_, subgraph| {
                if subgraph.is_expired_subgraph(block) || subgraph.ready_to_remove(block) {
                    tracing::debug!(pair=?p, "removing subgraph");
                    subgraph.get_all_pools().flatten().for_each(|edge| {
                        *removals.entry(edge.pool_addr).or_default() += 1;
                    });
                    self.metrics
                        .as_ref()
                        .inspect(|m| m.active_subgraphs.decrement(1.0));
                    return false
                }
                true
            });

            !vec.is_empty()
        });
        removals
    }

    /// finalize the block and move over the subgraphs for the given block into
    /// the active set.
    pub fn finalize_block(&mut self, block: u64) -> FastHashMap<Address, u64> {
        let mut removals = FastHashMap::default();
        if let Some(subgraphs) = self.pending_finalized_graphs.remove(&block) {
            subgraphs.sub_graphs.into_iter().for_each(|(pair, gts)| {
                for (gt, graph) in gts {
                    if let Some(old) = self
                        .sub_graphs
                        .entry(pair.ordered())
                        .or_default()
                        .insert(gt.ordered(), graph)
                    {
                        old.get_all_pools().flatten().for_each(|edge| {
                            *removals.entry(edge.pool_addr).or_default() += 1;
                        });
                    } else {
                        // not replacing
                        self.metrics
                            .as_ref()
                            .inspect(|m| m.active_subgraphs.increment(1.0));
                    }
                }
            });
        }
        removals
    }

    pub fn mark_future_use(&self, pair: Pair, gt: Pair, block: u64) {
        // we unwrap as this should never fail.
        let Some(graph) = self.sub_graphs.get(&pair.ordered()) else { return };
        if let Some(graph) = graph.get(&gt.ordered()) {
            graph.future_use(block);
        }
    }

    pub fn get_subgraph_extends_iter(
        &self,
        pair: PairWithFirstPoolHop,
    ) -> Vec<(PairWithFirstPoolHop, Option<Pair>)> {
        let pair = pair.get_pair();

        self.sub_graphs
            .get(&pair.ordered())
            .map(|graph| {
                graph
                    .iter()
                    .map(|(gt, inner)| (gt, inner.extends_to()))
                    .map(|(gt, ex)| (PairWithFirstPoolHop::from_pair_gt(pair, *gt), ex))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default()
    }

    pub fn get_subgraph_extends(&self, pair: PairWithFirstPoolHop) -> Option<Pair> {
        let (pair, gt) = pair.pair_gt();
        self.sub_graphs
            .get(&pair.ordered())
            .and_then(|graph| graph.get(&gt.ordered()).map(|s| s.extends_to()))
            .flatten()
    }

    pub fn all_pairs_with_quote(&self, addr: Address) -> Vec<Pair> {
        self.sub_graphs
            .iter()
            .filter(|(pair, _)| pair.1 == addr)
            .map(|(p, _)| *p)
            .collect_vec()
    }

    // check's to see if a subgraph that shares an edge exists.
    pub fn has_extension(&self, pair: &Pair, quote: Address) -> Option<Pair> {
        self.sub_graphs
            .iter()
            .find(|(cur_graphs, sub)| {
                cur_graphs.0 == pair.1
                    && cur_graphs.1 == quote
                    && sub.iter().all(|(_, s)| s.should_use_for_new())
            })
            .map(|(k, _)| k)
            .copied()
    }

    pub fn has_go_through(&self, pair: PairWithFirstPoolHop) -> bool {
        let (pair, gt) = pair.pair_gt();
        self.sub_graphs
            .get(&pair.ordered())
            .and_then(|s| s.get(&gt.ordered()).map(|s| s.should_use_for_new()))
            .or_else(|| {
                Some(
                    self.pending_finalized_graphs
                        .values()
                        .filter_map(|sg| {
                            sg.sub_graphs
                                .get(&pair.ordered())
                                .map(|s| s.get(&gt.ordered()).is_some())
                        })
                        .any(|f| f),
                )
                .filter(|f| *f)
            })
            .unwrap_or(false)
    }

    pub fn remove_subgraph(&mut self, pair: PairWithFirstPoolHop) -> FastHashMap<Address, u64> {
        let (pair, goes_through) = pair.pair_gt();

        let mut removals = FastHashMap::default();
        self.sub_graphs.retain(|k, v| {
            if k != &pair.ordered() {
                return true
            }
            v.retain(|gt, s| {
                let res = gt != &goes_through.ordered();
                if !res {
                    self.metrics
                        .as_ref()
                        .inspect(|m| m.active_subgraphs.decrement(1.0));
                    s.get_all_pools().flatten().for_each(|edge| {
                        *removals.entry(edge.pool_addr).or_default() += 1;
                    });
                }

                res
            });
            !v.is_empty()
        });
        removals
    }

    pub fn add_verified_subgraph(
        &mut self,
        mut subgraph: PairSubGraph,
        graph_state: FastHashMap<Address, &PoolState>,
        block: u64,
    ) {
        subgraph.save_last_verification_liquidity(&graph_state);

        if self
            .pending_finalized_graphs
            .entry(block)
            .or_default()
            .sub_graphs
            .entry(subgraph.complete_pair().ordered())
            .or_default()
            .insert(subgraph.must_go_through().ordered(), subgraph)
            .is_some()
        {
            tracing::warn!("double verified subgraph");
        }
    }

    pub fn verify_current_subgraphs<T: ProtocolState>(
        &mut self,
        pair: PairWithFirstPoolHop,
        start: Address,
        start_price: Rational,
        state: &FastHashMap<Address, &T>,
        block: u64,
    ) {
        let (pair, gt) = pair.pair_gt();
        self.sub_graphs.iter_mut().for_each(|(g_pair, sub)| {
            // wrong pair, then retain
            if *g_pair != pair.ordered() {
                return
            }

            sub.iter_mut().for_each(|(goes_through, graph)| {
                if *goes_through == gt {
                    graph.has_valid_liquidity(start, start_price.clone(), state, block)
                }
            });
        });
    }

    pub fn get_price(
        &mut self,
        unordered_pair: Pair,
        goes_through: Pair,
        edge_state: &FastHashMap<Address, &PoolState>,
    ) -> Option<(Rational, Rational, usize)> {
        let (next, complete_pair, default_price, connections, liq) =
            self.get_price_once(unordered_pair, goes_through, edge_state)?;

        if let Some(next) = next {
            // extend is assuemed stable
            let (next_price, ..) = self.get_price_all(next, edge_state)?;

            let price = next_price * &default_price;
            if unordered_pair.eq_unordered(&complete_pair) {
                Some((price, liq, connections))
            } else {
                Some((price.reciprocal(), liq, connections))
            }
        } else {
            Some((default_price, liq, connections))
        }
    }

    fn get_price_once(
        &self,
        unordered_pair: Pair,
        goes_through: Pair,
        edge_state: &FastHashMap<Address, &PoolState>,
    ) -> Option<(Option<Pair>, Pair, Rational, usize, Rational)> {
        let pair = unordered_pair.ordered();

        self.sub_graphs
            .get(&pair)
            .and_then(|g| g.get(&goes_through.ordered()))
            .map(|graph| {
                tracing::debug!("has graph for goes through");
                Some((
                    graph.extends_to(),
                    graph.complete_pair(),
                    graph.fetch_price(edge_state)?,
                    graph.first_hop_connections(),
                    graph.first_hop_min_liq(edge_state).unwrap_or_default(),
                ))
            })
            // this can happen when we have pools with a token that only has that one pool.
            // this causes a one way and we can't process price. Instead, in this case
            // we take the average price on non-extended graphs and return the price
            // that way
            .or_else(|| {
                Some(
                    self.get_price_all(unordered_pair, edge_state)
                        .map(|(price, con, e)| (None, unordered_pair, price, con, e)),
                )
            })
            .flatten()
    }

    /// for the given pair, grabs the price for all go-through variants
    pub(crate) fn get_price_all(
        &self,
        unordered_pair: Pair,
        edge_state: &FastHashMap<Address, &PoolState>,
    ) -> Option<(Rational, usize, Rational)> {
        let pair = unordered_pair.ordered();
        let mut connections = 0;
        let mut min_liq = Rational::ZERO;

        self.sub_graphs.get(&pair).and_then(|f| {
            let mut cnt = Rational::ZERO;
            let mut acc = Rational::ZERO;
            for graph in f.values() {
                if graph.extends_to().is_some() {
                    continue
                };

                let Some(next) = graph.fetch_price(edge_state) else {
                    continue;
                };

                if min_liq == Rational::ZERO {
                    min_liq = graph.first_hop_min_liq(edge_state).unwrap_or_default();
                } else if let Some(liq) = graph.first_hop_min_liq(edge_state) {
                    min_liq = std::cmp::min(min_liq, liq);
                }

                connections += graph.first_hop_connections();
                let default_pair = graph.get_unordered_pair();

                // ensure all graph pairs are accumulated in the same way
                acc += if !unordered_pair.eq_unordered(&default_pair) {
                    next.reciprocal()
                } else {
                    next
                };
                cnt += Rational::ONE;
            }
            (cnt != Rational::ZERO).then(|| (acc / cnt, connections, min_liq))
        })
    }
}
