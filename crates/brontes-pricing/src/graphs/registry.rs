use alloy_primitives::Address;
use brontes_types::{pair::Pair, price_graph_types::*, FastHashMap};
use malachite::{num::arithmetic::traits::Reciprocal, Rational};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

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
    sub_graphs: FastHashMap<Pair, PairSubGraph>,
}

impl SubGraphRegistry {
    pub fn new(subgraphs: FastHashMap<Pair, (Option<Pair>, Vec<SubGraphEdge>)>) -> Self {
        let sub_graphs = subgraphs
            .into_iter()
            .map(|(pair, (extends_to, edges))| {
                (pair.ordered(), PairSubGraph::init(pair, extends_to, edges))
            })
            .collect();

        Self { sub_graphs }
    }

    // check's to see if a subgraph that shares an edge exists.
    // if one exists, returns the pair.
    pub fn has_extension(&self, pair: &Pair) -> Option<Address> {
        self.sub_graphs
            .keys()
            .find_map(|cur_graphs| (cur_graphs.0 == pair.1).then_some(cur_graphs.0))
    }

    pub fn add_verified_subgraph(
        &mut self,
        pair: Pair,
        mut subgraph: PairSubGraph,
        graph_state: &FastHashMap<Address, PoolState>,
    ) {
        subgraph.save_last_verification_liquidity(graph_state);
        if self.sub_graphs.insert(pair.ordered(), subgraph).is_some() {
            tracing::error!(?pair, "already had a verified sub-graph for pair");
        }
    }

    /// looks through the subgraph for any pools that have had significant
    /// liquidity drops. when this occurs. removes the pair
    pub fn audit_subgraphs(&mut self, graph_state: &FastHashMap<Address, PoolState>) {
        let bad_graphs = self
            .sub_graphs
            .par_iter()
            .filter_map(
                |(pair, graph)| {
                    if graph.has_stale_liquidity(graph_state) {
                        Some(*pair)
                    } else {
                        None
                    }
                },
            )
            .collect::<Vec<_>>();

        self.sub_graphs.retain(|k, _| !bad_graphs.contains(k));
    }

    pub fn has_subpool(&self, pair: &Pair) -> bool {
        self.sub_graphs.contains_key(&pair.ordered())
    }

    pub fn get_price(
        &self,
        unordered_pair: Pair,
        edge_state: &FastHashMap<Address, PoolState>,
    ) -> Option<Rational> {
        let pair = unordered_pair.ordered();

        self.sub_graphs
            .get(&pair)
            .map(|graph| (graph.get_unordered_pair(), graph))
            .and_then(|(default_pair, graph)| Some((default_pair, graph.fetch_price(edge_state)?)))
            .map(
                |(default_pair, res)| {
                    if !unordered_pair.eq_unordered(&default_pair) {
                        res.reciprocal()
                    } else {
                        res
                    }
                },
            )
    }
}
