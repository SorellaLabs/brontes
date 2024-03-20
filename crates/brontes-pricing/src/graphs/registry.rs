use alloy_primitives::Address;
use brontes_types::{pair::Pair, price_graph_types::*, FastHashMap};
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
#[derive(Debug)]
pub struct SubGraphRegistry {
    /// all currently known sub-graphs
    sub_graphs: FastHashMap<Pair, Vec<(Pair, PairSubGraph)>>,
}

impl SubGraphRegistry {
    pub fn new(subgraphs: FastHashMap<Pair, (Pair, Option<Pair>, Vec<SubGraphEdge>)>) -> Self {
        let sub_graphs = subgraphs
            .into_iter()
            .map(|(pair, (goes_through, extends_to, edges))| {
                (
                    pair.ordered(),
                    vec![(goes_through, PairSubGraph::init(goes_through, pair, extends_to, edges))],
                )
            })
            .collect();

        Self { sub_graphs }
    }

    // check's to see if a subgraph that shares an edge exists.
    // if one exists, returns the pair.
    pub fn has_extension(&self, pair: &Pair) -> Option<Pair> {
        self.sub_graphs
            .keys()
            .find(|cur_graphs| cur_graphs.0 == pair.1)
            .copied()
    }

    pub fn has_go_through(&self, pair: &Pair, goes_through: &Pair) -> bool {
        self.sub_graphs
            .get(pair)
            .filter(|g| g.iter().find(|(gt, _)| gt == goes_through).is_some())
            .is_some()
    }

    pub fn add_verified_subgraph(
        &mut self,
        pair: Pair,
        mut subgraph: PairSubGraph,
        graph_state: &FastHashMap<Address, PoolState>,
    ) {
        subgraph.save_last_verification_liquidity(graph_state);
        self.sub_graphs
            .entry(pair.ordered())
            .or_insert_with(|| vec![])
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

    pub fn get_price(
        &self,
        unordered_pair: Pair,
        goes_through: Pair,
        edge_state: &FastHashMap<Address, PoolState>,
    ) -> Option<Rational> {
        let (next, default_price) =
            self.get_price_once(unordered_pair, goes_through, edge_state)?;

        next.and_then(|next| Some(self.get_price_all(next, edge_state)? * &default_price))
            .or(Some(default_price))
    }

    fn get_price_once(
        &self,
        unordered_pair: Pair,
        goes_through: Pair,
        edge_state: &FastHashMap<Address, PoolState>,
    ) -> Option<(Option<Pair>, Rational)> {
        let pair = unordered_pair.ordered();

        self.sub_graphs
            .get(&pair)
            .and_then(|f| {
                f.iter()
                    .find_map(|(gt, graph)| (*gt == goes_through).then_some(graph))
            })
            .map(|graph| (graph.get_unordered_pair(), graph))
            .and_then(|(default_pair, graph)| {
                Some((graph.extends_to(), default_pair, graph.fetch_price(edge_state)?))
            })
            .map(|(ext, default_pair, res)| {
                if !unordered_pair.eq_unordered(&default_pair) {
                    (ext, res.reciprocal())
                } else {
                    (ext, res)
                }
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
