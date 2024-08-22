use std::collections::BTreeMap;

use alloy_primitives::Address;
use brontes_metrics::pricing::DexPricingMetrics;
use brontes_types::{pair::Pair, FastHashMap, FastHashSet};
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

            // ensure if subgraph extends others that those get marked
            if let Some(extends_to) = graph.extends_to() {
                self.sub_graphs.get(&extends_to.ordered()).map(|subgraphs| {
                    subgraphs.values().for_each(|sub_g| sub_g.future_use(block));
                });
            }
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

    pub fn all_pairs_with_quote_for_extends(&self, addr: Address) -> Vec<Pair> {
        self.sub_graphs
            .iter()
            .filter(|(pair, _)| pair.1 == addr)
            .filter(|(_, subgraphs)| {
                subgraphs
                    .iter()
                    .any(|(_, subgraph)| subgraph.should_use_for_extend())
            })
            .map(|(p, _)| *p)
            .collect_vec()
    }

    // check's to see if a subgraph that shares an edge exists.
    pub fn has_extension(&self, pair: &Pair, quote: Address) -> Option<Pair> {
        self.sub_graphs
            .iter()
            .find(|(cur_graphs, sub)| {
                // if start key == end key of first
                cur_graphs.0 == pair.1
                    // if end key of cur is quote
                    && cur_graphs.1 == quote
                    // can be used to extend
                    && sub.iter().any(|(_, s)| s.should_use_for_extend())
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

    pub fn mark_subgraph_for_removal(&mut self, pair: PairWithFirstPoolHop, block: u64) {
        let (pair, goes_through) = pair.pair_gt();

        self.sub_graphs.get_mut(&pair.ordered()).map(|v| {
            if let Some(subgraph) = v.get_mut(&goes_through.ordered()) {
                subgraph.remove_at = Some(block);
            }
        });
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

    // returns a set of pairs that can no longer be used to extend
    pub fn verify_current_subgraphs<T: ProtocolState>(
        &mut self,
        args: Vec<(PairWithFirstPoolHop, Address, Rational)>,
        state: &FastHashMap<Address, &T>,
        block: u64,
    ) {
        let mut invalid_extends = FastHashSet::default();

        for (pair, start, start_price) in args {
            let (pair, gt) = pair.pair_gt();
            if let Some(range) = self.sub_graphs.get_mut(&pair.ordered()) {
                if let Some(graph) = range.get_mut(&gt.ordered()) {
                    if !graph.has_valid_liquidity(start, start_price.clone(), state, block) {
                        // if we extend to another subgraph, then we dont gotta check.
                        if graph.extends_to().is_some() {
                            return
                        }

                        let possible_bad_extends_to = graph.pair;
                        let valid_count = range
                            .iter()
                            // keep if can be base and isn't being removed
                            .filter(|(_, g)| g.extends_to().is_none() && g.remove_at.is_none())
                            .count();

                        if valid_count == 0 {
                            invalid_extends.insert(possible_bad_extends_to.ordered());
                        }
                    }
                }
            }
        }

        if !invalid_extends.is_empty() {
            // active
            self.sub_graphs.iter_mut().for_each(|(_, graphs)| {
                graphs.iter_mut().for_each(|(_, graph)| {
                    if let Some(extends) = graph.extends_to() {
                        if invalid_extends.contains(&extends.ordered()) {
                            tracing::info!(target: "brontes::missing_pricing", "avoided pointing to nil");
                            graph.remove_at = Some(block);
                        }
                    }
                })
            });

            // inactive
            self.pending_finalized_graphs.values_mut().for_each(|sub| {
                sub.sub_graphs.iter_mut().for_each(|(_, graphs)| {
                graphs.iter_mut().for_each(|(_, graph)| {
                    if let Some(extends) = graph.extends_to() {
                        if invalid_extends.contains(&extends.ordered()) {
                            tracing::info!(target: "brontes::missing_pricing", "avoided pointing to nil");
                            graph.remove_at = Some(block);
                        }
                    }
                })
            });

            });
        }
    }

    pub fn get_price(
        &self,
        unordered_pair: Pair,
        goes_through: Pair,
        edge_state: &FastHashMap<Address, &PoolState>,
    ) -> Option<Rational> {
        let (next, complete_pair, default_price) =
            self.get_price_once(unordered_pair, goes_through, edge_state)?;

        if let Some(next) = next {
            let Some(next_price) = self.get_price_all(next, edge_state) else {
                tracing::info!(target:"brontes::missing_pricing",
                    pair=?next,
                    "subgraph that extends other points to nil"
                );

                return None
            };

            let price = next_price * &default_price;
            if unordered_pair.eq_unordered(&complete_pair) {
                Some(price)
            } else {
                Some(price.reciprocal())
            }
        } else {
            Some(default_price)
        }
    }

    fn get_price_once(
        &self,
        unordered_pair: Pair,
        goes_through: Pair,
        edge_state: &FastHashMap<Address, &PoolState>,
    ) -> Option<(Option<Pair>, Pair, Rational)> {
        let pair = unordered_pair.ordered();

        self.sub_graphs
            .get(&pair)
            .and_then(|g| g.get(&goes_through.ordered()))
            .map(|graph| {
                tracing::debug!("has graph for goes through");
                Some((graph.extends_to(), graph.complete_pair(), graph.fetch_price(edge_state)?))
            })
            // this can happen when we have pools with a token that only has that one pool.
            // this causes a one way and we can't process price. Instead, in this case
            // we take the average price on non-extended graphs and return the price
            // that way
            .or_else(|| {
                Some(
                    self.get_price_all(unordered_pair, edge_state)
                        .map(|price| (None, unordered_pair, price)),
                )
            })
            .flatten()
    }

    /// for the given pair, grabs the price for all go-through variants
    pub(crate) fn get_price_all(
        &self,
        unordered_pair: Pair,
        edge_state: &FastHashMap<Address, &PoolState>,
    ) -> Option<Rational> {
        let pair = unordered_pair.ordered();

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

                let default_pair = graph.get_unordered_pair();

                // ensure all graph pairs are accumulated in the same way
                acc += if !unordered_pair.eq_unordered(&default_pair) {
                    next.reciprocal()
                } else {
                    next
                };
                cnt += Rational::ONE;
            }
            (cnt != Rational::ZERO).then(|| acc / cnt).or_else(|| {
                tracing::info!("get_price_all failed to fetch price");
                None
            })
        })
    }
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
