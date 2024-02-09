use std::collections::{HashMap, HashSet};

use alloy_primitives::Address;
use brontes_types::{pair::Pair, price_graph_types::*, Protocol};
use itertools::Itertools;
use malachite::{num::arithmetic::traits::Reciprocal, Rational};

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
    /// tracks which tokens have a edge in the subgraph,
    /// this allows us to possibly insert a new node to a subgraph
    /// if it fits the criteria
    token_to_sub_graph: HashMap<Address, HashSet<Pair>>,
    /// all currently known sub-graphs
    sub_graphs:         HashMap<Pair, PairSubGraph>,
}

impl SubGraphRegistry {
    pub fn new(subgraphs: HashMap<Pair, Vec<SubGraphEdge>>) -> Self {
        let mut token_to_sub_graph: HashMap<Address, HashSet<Pair>> = HashMap::new();
        let sub_graphs = subgraphs
            .into_iter()
            .map(|(pair, edges)| {
                edges
                    .iter()
                    .flat_map(|e| vec![e.token_0, e.token_1])
                    .for_each(|token| {
                        token_to_sub_graph.entry(token).or_default().insert(pair);
                    });

                (pair.ordered(), PairSubGraph::init(pair, edges))
            })
            .collect();
        Self { token_to_sub_graph, sub_graphs }
    }

    pub fn add_verified_subgraph(&mut self, pair: Pair, subgraph: PairSubGraph) {
        // add all tokens
        subgraph
            .get_all_pools()
            .flat_map(|e| {
                e.iter()
                    .flat_map(|e| vec![e.token_0, e.token_1])
                    .collect_vec()
            })
            .unique()
            .for_each(|token| {
                self.token_to_sub_graph
                    .entry(token)
                    .or_default()
                    .insert(pair.ordered());
            });

        if self.sub_graphs.insert(pair.ordered(), subgraph).is_some() {
            tracing::error!(?pair, "already had a verified sub-graph for pair");
        }
    }

    pub fn has_subpool(&self, pair: &Pair) -> bool {
        self.sub_graphs.contains_key(&pair.ordered())
    }

    pub fn bad_pool_state(
        &mut self,
        subgraph: Pair,
        pool_pair: Pair,
        pool_address: Address,
    ) -> bool {
        let Some(mut graph) = self.sub_graphs.remove(&subgraph.ordered()) else { return true };

        let is_disjoint_graph = graph.remove_bad_node(pool_pair, pool_address);
        if !is_disjoint_graph {
            self.sub_graphs.insert(subgraph.ordered(), graph);
        } else {
            // remove pair from token lookup
            self.token_to_sub_graph.retain(|_, v| {
                v.remove(&subgraph.ordered());
                !v.is_empty()
            });
        }

        is_disjoint_graph
    }

    #[allow(unused)]
    pub fn try_extend_subgraphs(
        &mut self,
        pool_address: Address,
        dex: Protocol,
        pair: Pair,
    ) -> Vec<(Pair, Vec<SubGraphEdge>)> {
        let token_0 = pair.0;
        let token_1 = pair.1;

        let Some(t0_subgraph) = self.token_to_sub_graph.get(&token_0) else { return vec![] };
        let Some(t1_subgraph) = self.token_to_sub_graph.get(&token_1) else { return vec![] };

        t0_subgraph
            .intersection(t1_subgraph)
            .map(|subgraph_pair| {
                (
                    subgraph_pair,
                    PoolPairInformation {
                        pool_addr: pool_address,
                        dex_type:  dex,
                        token_0:   pair.0,
                        token_1:   pair.1,
                    },
                )
            })
            .filter_map(|(pair, info)| {
                if let Some(subgraph) = self.sub_graphs.get_mut(pair) {
                    if subgraph.add_new_edge(info) {
                        return Some((
                            *pair,
                            subgraph.get_all_pools().flatten().cloned().collect_vec(),
                        ))
                    }
                }
                None
            })
            .collect_vec()
    }

    pub fn get_price(
        &self,
        unordered_pair: Pair,
        edge_state: &HashMap<Address, PoolState>,
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
