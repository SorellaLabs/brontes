use std::collections::{HashMap, HashSet};

use alloy_primitives::Address;
use brontes_types::pair::Pair;
use itertools::Itertools;
use malachite::{num::arithmetic::traits::Reciprocal, Rational};
use rayon::iter::{IntoParallelIterator, ParallelIterator};

use super::{subgraph::PairSubGraph, PoolState};
use crate::{price_graph_types::*, types::PoolUpdate, AllPairGraph, Protocol};

/// stores all sub-graphs and supports the update mechanisms
#[derive(Debug, Clone)]
pub struct SubGraphRegistry {
    /// tracks which tokens have a edge in the subgraph,
    /// this allows us to possibly insert a new node to a subgraph
    /// if it fits the criteria
    token_to_sub_graph: HashMap<Address, HashSet<Pair>>,
    /// all currently known sub-graphs
    sub_graphs:         HashMap<Pair, PairSubGraph>,
    /// This is used to store a given pools tvl.
    /// we do this here so that all subpools just have a pointer
    /// to this data which allows us to not worry about updating all subgraphs
    /// when the tvl of a pool changes.
    /// pool address -> pool tvl
    edge_state:         HashMap<Address, PoolState>,
    /// subgraphs that needed to be requeried. we keep track of these
    requeried_graphs:   HashSet<Pair>,
}

impl SubGraphRegistry {
    pub fn new(subgraphs: HashMap<Pair, Vec<SubGraphEdge>>) -> Self {
        let mut token_to_sub_graph: HashMap<Address, HashSet<Pair>> = HashMap::new();
        let sub_graphs = subgraphs
            .into_iter()
            .map(|(pair, edges)| {
                // add to lookup
                println!("subgraph");
                edges
                    .iter()
                    .flat_map(|e| vec![e.token_0, e.token_1])
                    .for_each(|token| {
                        token_to_sub_graph.entry(token).or_default().insert(pair);
                    });

                (pair, PairSubGraph::init(pair, edges))
            })
            .collect();
        Self {
            token_to_sub_graph,
            sub_graphs,
            edge_state: HashMap::default(),
            requeried_graphs: HashSet::new(),
        }
    }

    pub fn has_subpool(&self, pair: &Pair) -> bool {
        self.sub_graphs.contains_key(&pair) || self.requeried_graphs.contains(&pair)
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

    pub fn fetch_unloaded_state(&self, pair: &Pair) -> Vec<PoolPairInfoDirection> {
        let Some(graph) = self.sub_graphs
            .get(&pair.ordered()) else { return vec![] };

        graph
            .get_all_pools()
            .flatten()
            .filter(|pool| !self.edge_state.contains_key(&pool.pool_addr))
            .map(|pool| pool.info)
            .collect_vec()
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

    pub fn all_unloaded_state(&self, edges: &Vec<SubGraphEdge>) -> Vec<PoolPairInfoDirection> {
        edges
            .into_iter()
            .filter(|e| !self.edge_state.contains_key(&e.pool_addr))
            .map(|f| f.info)
            .collect_vec()
    }

    pub fn create_new_subgraph(
        &mut self,
        pair: Pair,
        path: Vec<SubGraphEdge>,
    ) -> Vec<PoolPairInfoDirection> {
        self.requeried_graphs.remove(&pair.ordered());
        // all edges
        let unloaded_state = path
            .iter()
            .filter(|e| !self.edge_state.contains_key(&e.pool_addr))
            .map(|f| f.info)
            .collect_vec();

        // add to sub_graph lookup
        let tokens = path
            .iter()
            .flat_map(|i| [i.token_0, i.token_1])
            .collect::<HashSet<_>>();

        tokens.into_iter().for_each(|token| {
            self.token_to_sub_graph
                .entry(token)
                .or_default()
                .insert(pair);
        });
        // init subgraph
        let subgraph = PairSubGraph::init(pair, path);
        self.sub_graphs.insert(pair, subgraph);

        unloaded_state
    }

    pub fn update_pool_state(&mut self, pool_address: Address, update: PoolUpdate) {
        self.edge_state
            .get_mut(&pool_address)
            .map(|state| state.increment_state(update));
    }

    pub fn new_pool_state(
        &mut self,
        address: Address,
        state: PoolState,
    ) -> Vec<(Pair, Vec<SubGraphEdge>)> {
        let dex = state.dex();
        let pair = state.pair();
        self.edge_state.insert(address, state);
        self.try_extend_subgraphs(address, dex, pair)
    }

    pub fn get_price(&self, unordered_pair: Pair) -> Option<Rational> {
        let (swapped, pair) = unordered_pair.ordered_changed();

        self.sub_graphs
            .get(&pair)
            .and_then(|graph| graph.fetch_price(&self.edge_state))
            .map(|res| if swapped { res.reciprocal() } else { res })
    }

    pub fn has_state(&self, addr: &Address) -> Option<u64> {
        self.edge_state.get(addr).map(|state| state.last_update)
    }

    // goes through the subgraph verifying that we have more than
    // the base amount of liquidity that we defined.
    // If we don't have enough defined, the pool is removed.
    // we return all bad nodes to be pruned from our all_pairs graph.
    // along with a bool if this pair needs to be recalculated.
    pub fn verify_subgraph(
        &mut self,
        pair: Vec<(u64, Pair)>,
        quote: Address,
        all_graph: &AllPairGraph,
    ) -> Vec<(bool, u64, Pair, HashMap<Pair, Vec<Address>>)> {
        let pairs = pair
            .into_iter()
            .map(|(block, pair)| (pair, block, self.sub_graphs.remove(&pair.ordered())))
            .filter_map(|(pair, block, subgraph)| {
                let Some(subgraph) = subgraph else { 
                    self.token_to_sub_graph.retain(|_, v| {
                        v.remove(&pair);
                        !v.is_empty()
                });

                    return None 
                };
                Some((pair, block, subgraph))
            })
            .collect_vec();

        let res = pairs
            .into_par_iter()
            .filter_map(|(pair, block, mut subgraph)| {
                let (bad, state) = subgraph.verify_subgraph(quote, &self.edge_state, all_graph);
                Some((pair, bad, block, state, subgraph))
            })
            .collect::<Vec<_>>();

        res.into_iter()
            .map(|(pair, kill, block, state, subgraph)| {
                if !kill {
                    self.sub_graphs.insert(pair.ordered(), subgraph);
                } else {
                    self.requeried_graphs.insert(pair.ordered());
                }

                self.token_to_sub_graph.retain(|_, v| {
                    v.remove(&pair.ordered());
                    !v.is_empty()
                });

                (kill, block, pair, state)
            })
            .collect_vec()
    }
}
