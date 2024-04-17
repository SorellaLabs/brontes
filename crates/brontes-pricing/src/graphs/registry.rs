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
                        PairSubGraph::init(pair, complete_pair, goes_through, extends_to, edges),
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
            .keys()
            .find(|cur_graphs| cur_graphs.0 == pair.1 && cur_graphs.1 == quote)
            .copied()
    }

    pub fn has_go_through(&self, pair: &Pair, goes_through: &Pair) -> bool {
        self.sub_graphs
            .get(&pair.ordered())
            .filter(|g| g.iter().any(|(gt, _)| gt == goes_through))
            .is_some()
    }

    pub fn remove_subgraph(&mut self, pair: &Pair, goes_through: &Pair) {
        self.sub_graphs.retain(|k, v| {
            if k != pair {
                return true
            }
            v.retain(|(gt, _)| gt != goes_through);
            !v.is_empty()
        });
    }

    pub fn add_verified_subgraph(
        &mut self,
        mut subgraph: PairSubGraph,
        graph_state: &FastHashMap<Address, PoolState>,
    ) {
        subgraph.save_last_verification_liquidity(graph_state);
        self.sub_graphs
            .entry(subgraph.complete_pair().ordered())
            .or_default()
            .push((subgraph.must_go_through(), subgraph));
    }

    /// looks through the subgraph for any pools that have had significant
    /// liquidity drops. when this occurs. removes the pair
    pub fn audit_subgraphs(&mut self, graph_state: &FastHashMap<Address, PoolState>) {
        self.sub_graphs.retain(|_, v| {
            v.retain(|(_, sub)| !sub.has_stale_liquidity(graph_state));
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
        state: &FastHashMap<Address, T>,
    ) -> Option<bool> {
        let mut requery = false;
        self.sub_graphs.retain(|g_pair, sub| {
            // wrong pair, then retain
            if *g_pair != pair.ordered() {
                return true
            }

            sub.retain_mut(|(gt, graph)| {
                if goes_through == gt {
                    let res = graph.rundown_subgraph_check(start, start_price.clone(), state);
                    // shit is disjoint
                    if res.should_abandon {
                        requery = true;
                        return false
                    }
                }
                true
            });

            !sub.is_empty()
        });

        Some(requery)
    }

    pub fn get_price(
        &mut self,
        unordered_pair: Pair,
        goes_through: Pair,
        goes_through_address: Option<Address>,
        edge_state: &FastHashMap<Address, PoolState>,
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
        edge_state: &FastHashMap<Address, PoolState>,
    ) -> Option<(Option<Pair>, Pair, Rational)> {
        let pair = unordered_pair.ordered();

        self.sub_graphs
            .get(&pair)
            .and_then(|g| {
                tracing::debug!(?unordered_pair, "has subgraph");
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
                tracing::debug!(?unordered_pair, ?goes_through, "trying price all");
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
        edge_state: &FastHashMap<Address, PoolState>,
    ) -> Option<Rational> {
        let pair = unordered_pair.ordered();

        self.sub_graphs.get(&pair).and_then(|f| {
            let mut cnt = Rational::ZERO;
            let mut acc = Rational::ZERO;
            for (p, graph) in f {
                if graph.extends_to().is_some() {
                    tracing::debug!(extends=?p,"price all etends_to is some");
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
