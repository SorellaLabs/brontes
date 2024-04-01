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

    pub fn has_go_through(&self, pair: &Pair, goes_through: &Option<Pair>) -> bool {
        if let Some(goes_through) = goes_through {
            self.sub_graphs
                .get(pair)
                .filter(|g| {
                    g.iter()
                        .any(|(gt, _)| gt == goes_through || goes_through.is_zero())
                })
                .is_some()
        } else {
            self.sub_graphs.contains_key(pair)
        }
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

    // if we have more than 4 extensions, this is enough of a market outlook
    pub fn current_pairs(&self, pair: &Pair) -> usize {
        self.sub_graphs
            .get(pair)
            .map(|f| f.len())
            .unwrap_or_default()
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
            inner.retain(|(_, g)| g.extends_to().map(|ex| ex != pair).unwrap_or(true));
            !inner.is_empty()
        })
    }

    pub fn get_price(
        &mut self,
        unordered_pair: Pair,
        goes_through: Pair,
        edge_state: &FastHashMap<Address, PoolState>,
    ) -> Option<Rational> {
        let (next, complete_pair, default_price) =
            self.get_price_once(unordered_pair, goes_through, edge_state)?;

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
        edge_state: &FastHashMap<Address, PoolState>,
    ) -> Option<(Option<Pair>, Pair, Rational)> {
        let pair = unordered_pair.ordered();

        self.sub_graphs
            .get(&pair)
            .and_then(|g| {
                g.iter().find_map(|(gt, graph)| {
                    (*gt == goes_through || gt.flip() == goes_through).then_some(graph)
                })
            })
            .and_then(|graph| {
                Some((graph.extends_to(), graph.complete_pair(), graph.fetch_price(edge_state)?))
            })
            // this can happen when we have pools with a token that only has that one pool.
            // this causes a one way and we can't process price. Instead, in this case
            // we take the average price on non-extended graphs and return the price
            // that way
            .or_else(|| {
                self.get_price_all(unordered_pair, edge_state)
                    .map(|price| (None, unordered_pair, price))
            })
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
            for (_, graph) in f {
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
            (cnt != Rational::ZERO).then(|| acc / cnt)
        })
    }
}
