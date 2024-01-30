use std::collections::{HashMap, HashSet};

use alloy_primitives::Address;
use brontes_types::pair::Pair;
use itertools::Itertools;
use malachite::{num::arithmetic::traits::Reciprocal, Rational};

use super::{subgraph::PairSubGraph, PoolState};
use crate::{price_graph_types::*, Protocol};

/// stores all sub-graphs and supports the update mechanisms
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

                (pair, PairSubGraph::init(pair, edges))
            })
            .collect();
        Self { token_to_sub_graph, sub_graphs }
    }

    pub fn add_verified_subgraph(&mut self, pair: Pair, subgraph: PairSubGraph) {
        // add all tokens
        subgraph
            .get_all_pools()
            .flat_map(|e| {
                e.into_iter()
                    .flat_map(|e| vec![e.token_0, e.token_1])
                    .collect_vec()
            })
            .unique()
            .for_each(|token| {
                self.token_to_sub_graph
                    .entry(token)
                    .or_default()
                    .insert(pair);
            });

        if self.sub_graphs.insert(pair.ordered(), subgraph).is_some() {
            tracing::error!(?pair, "already had a verified sub-graph for pair");
        }
    }

    pub fn has_subpool(&self, pair: &Pair) -> bool {
        self.sub_graphs.contains_key(&pair)
    }

    pub fn bad_pool_state(
        &mut self,
        subgraph: Pair,
        pool_pair: Pair,
        pool_address: Address,
    ) -> bool {
        let Some(mut graph) = self.sub_graphs.remove(&subgraph) else { return true };

        let is_disjoint_graph = graph.remove_bad_node(pool_pair, pool_address);
        if !is_disjoint_graph {
            self.sub_graphs.insert(subgraph, graph);
        } else {
            // remove pair from token lookup
            self.token_to_sub_graph.retain(|_, v| {
                v.remove(&subgraph);
                !v.is_empty()
            });
        }

        is_disjoint_graph
    }

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
        let (swapped, pair) = unordered_pair.ordered_changed();

        self.sub_graphs
            .get(&pair)
            .and_then(|graph| graph.fetch_price(edge_state))
            .map(|res| if swapped { res.reciprocal() } else { res })
    }
}
