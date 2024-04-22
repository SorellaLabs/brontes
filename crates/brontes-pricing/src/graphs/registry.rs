use alloy_primitives::Address;
use brontes_types::{pair::Pair, price_graph_types::*, FastHashMap};
use itertools::Itertools;
use malachite::{
    num::{
        arithmetic::traits::Reciprocal,
        basic::traits::{One, Zero},
    },
    Rational,
};

use super::{subgraph::PairSubGraph, PoolState};
use crate::types::ProtocolState;

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
    sub_graphs: FastHashMap<Pair, Vec<(Pair, PairSubGraph)>>,
}

impl SubGraphRegistry {
    pub fn new(
        subgraphs: FastHashMap<Pair, (Pair, Pair, Option<Pair>, Vec<SubGraphEdge>)>,
    ) -> Self {
        let sub_graphs = subgraphs
            .into_iter()
            .map(|(pair, (goes_through, complete_pair, extends_to, edges))| {
                (
                    pair.ordered(),
                    vec![(
                        goes_through,
                        PairSubGraph::init(pair, complete_pair, goes_through, extends_to, edges, 0),
                    )],
                )
            })
            .collect();

        Self { sub_graphs }
    }

    // useful for debugging
    #[allow(unused)]
    pub fn check_for_dups(&self) {
        self.sub_graphs
            .iter()
            .flat_map(|(a, v)| v.iter().zip(vec![a].into_iter().cycle()))
            .map(|(inner, out)| (out.ordered(), inner.0))
            .counts()
            .iter()
            .for_each(|((pair, extends), am)| {
                if *am != 1 {
                    tracing::warn!(
                        ?pair,
                        ?extends,
                        amount=?am,
                        "has more than one entry in the subgraph registry"
                    );
                }
            });
    }

    // for all subgraphs that haven't been used in a given time period, will
    // remove them from and return each pool with the amount to decrement.
    pub fn prune_dead_subgraphs(&mut self, block: u64) -> FastHashMap<Address, u64> {
        let mut removals = FastHashMap::default();

        self.sub_graphs.retain(|_, vec| {
            vec.retain(|(_, subgraph)| {
                if subgraph.is_expired_subgraph(block) || subgraph.ready_to_remove(block) {
                    subgraph.get_all_pools().flatten().for_each(|edge| {
                        *removals.entry(edge.pool_addr).or_default() += 1;
                    });
                    return false
                }
                true
            });

            !vec.is_empty()
        });
        removals
    }

    pub fn mark_future_use(&self, pair: &Pair, goes_through: &Pair, block: u64) {
        // we unwrap as this should never fail.
        let graph = self.sub_graphs.get(&pair.ordered()).unwrap();
        graph
            .iter()
            .find(|(inner_gt, _)| (inner_gt == goes_through))
            .unwrap()
            .1
            .future_use(block);
    }

    pub fn get_subgraph_extends(&self, pair: &Pair, goes_through: &Pair) -> Option<Pair> {
        self.sub_graphs
            .get(&pair.ordered())
            .and_then(|graph| {
                graph
                    .iter()
                    .find_map(|(inner_gt, s)| (inner_gt == goes_through).then(|| s.extends_to()))
            })
            .flatten()
    }

    pub fn all_pairs_with_quote(&self, addr: Address) -> Vec<Pair> {
        self.sub_graphs
            .keys()
            .copied()
            .filter(|pair| pair.1 == addr)
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

    pub fn has_go_through(&self, pair: &Pair, goes_through: &Pair) -> bool {
        self.sub_graphs
            .get(&pair.ordered())
            .filter(|g| {
                g.iter()
                    .any(|(gt, s)| gt == goes_through && s.should_use_for_new())
            })
            .is_some()
    }

    pub fn remove_subgraph(
        &mut self,
        pair: &Pair,
        goes_through: &Pair,
    ) -> FastHashMap<Address, u64> {
        let mut removals = FastHashMap::default();
        self.sub_graphs.retain(|k, v| {
            if k != pair {
                return true
            }
            v.retain(|(gt, s)| {
                let res = gt != goes_through;
                if !res {
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
    ) {
        subgraph.save_last_verification_liquidity(&graph_state);

        self.sub_graphs
            .entry(subgraph.complete_pair().ordered())
            .or_default()
            .push((subgraph.must_go_through(), subgraph));
    }

    /// looks through the subgraph for any pools that have had significant
    /// liquidity drops. when this occurs. removes the pair
    pub fn audit_subgraphs(&mut self, graph_state: FastHashMap<Address, &PoolState>) {
        self.sub_graphs.retain(|_, v| {
            v.retain(|(_, sub)| !sub.has_stale_liquidity(&graph_state));
            !v.is_empty()
        });
    }

    pub fn has_subpool(&self, pair: &Pair) -> bool {
        self.sub_graphs.contains_key(&pair.ordered())
    }

    fn remove_all_extensions_of(&mut self, pair: Pair) {
        self.sub_graphs.retain(|_, inner| {
            inner.retain(|(_, g)| {
                g.extends_to()
                    .map(|ex| ex.ordered() != pair.ordered())
                    .unwrap_or(true)
            });
            !inner.is_empty()
        })
    }

    pub fn verify_current_subgraphs<T: ProtocolState>(
        &mut self,
        pair: Pair,
        goes_through: &Pair,
        start: Address,
        start_price: Rational,
        state: &FastHashMap<Address, &T>,
        block: u64,
    ) {
        self.sub_graphs.iter_mut().for_each(|(g_pair, sub)| {
            // wrong pair, then retain
            if *g_pair != pair.ordered() {
                return
            }

            sub.iter_mut().for_each(|(gt, graph)| {
                if goes_through == gt {
                    graph.has_valid_liquidity(start, start_price.clone(), state, block)
                }
            });
        });
    }

    pub fn get_price(
        &mut self,
        unordered_pair: Pair,
        goes_through: Pair,
        goes_through_address: Option<Address>,
        edge_state: &FastHashMap<Address, &PoolState>,
    ) -> Option<Rational> {
        let (next, complete_pair, default_price) =
            self.get_price_once(unordered_pair, goes_through, goes_through_address, edge_state)?;

        if let Some(next) = next {
            let next_price = self.get_price_all(next, edge_state);
            if next_price.is_none() {
                self.remove_all_extensions_of(next);
                return None
            }

            let price = next_price.unwrap() * &default_price;
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
        goes_through_address: Option<Address>,
        edge_state: &FastHashMap<Address, &PoolState>,
    ) -> Option<(Option<Pair>, Pair, Rational)> {
        let pair = unordered_pair.ordered();

        self.sub_graphs
            .get(&pair)
            .and_then(|g| {
                g.iter()
                    .find_map(|(gt, graph)| (*gt == goes_through).then_some(graph))
            })
            .map(|graph| {
                Some((
                    graph.extends_to(),
                    graph.complete_pair(),
                    graph.fetch_price(edge_state, goes_through_address)?,
                ))
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
            for (_, graph) in f {
                if graph.extends_to().is_some() || !graph.should_use_for_new() {
                    continue
                };

                let Some(next) = graph.fetch_price(edge_state, None) else {
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
            (cnt != Rational::ZERO).then(|| acc / cnt)
        })
    }
}
