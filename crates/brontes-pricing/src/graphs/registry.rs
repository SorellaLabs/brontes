use std::collections::{HashMap, HashSet};

use alloy_primitives::Address;
use brontes_types::{exchanges::StaticBindingsDb, extra_processing::Pair};
use indexmap::set::Intersection;
use itertools::Itertools;
use malachite::{num::arithmetic::traits::Reciprocal, Rational};

use super::{
    subgraph::{PairSubGraph, SubGraphEdge},
    PoolPairInfoDirection, PoolState,
};
use crate::types::{PoolStateSnapShot, PoolUpdate};

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
}

impl SubGraphRegistry {
    pub fn new(subgraphs: HashMap<Pair, Vec<SubGraphEdge>>) -> Self {
        let mut token_to_sub_graph: HashMap<Address, HashSet<Pair>> = HashMap::new();
        let sub_graphs = subgraphs
            .into_iter()
            .map(|(pair, edges)| {
                // add to lookup
                edges
                    .iter()
                    .flat_map(|e| vec![e.token_0, e.token_1])
                    .for_each(|token| {
                        token_to_sub_graph.entry(token).or_default().insert(pair);
                    });

                (pair, PairSubGraph::init(pair, edges))
            })
            .collect();
        Self { token_to_sub_graph, sub_graphs, edge_state: HashMap::default() }
    }

    pub fn has_subpool(&self, pair: &Pair) -> bool {
        self.sub_graphs.contains_key(&pair.ordered())
    }

    pub fn fetch_unloaded_state(&self, pair: &Pair) -> Vec<PoolPairInfoDirection> {
        self.sub_graphs
            .get(&pair.ordered())
            .unwrap()
            .get_all_pools()
            .flatten()
            .filter(|pool| !self.edge_state.contains_key(&pool.pool_addr))
            .map(|pool| pool.info)
            .collect_vec()
    }

    pub fn try_extend_subgraphs(
        &mut self,
        pool_address: Address,
        dex: StaticBindingsDb,
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
                    super::PoolPairInformation {
                        pool_addr: pool_address,
                        dex_type:  dex,
                        token_0:   pair.0,
                        token_1:   pair.1,
                    },
                )
            })
            .map(|(pair, info)| {
                let subgraph = self.sub_graphs.get_mut(pair).unwrap();
                subgraph.add_new_edge(info);
                (*pair, subgraph.get_all_pools().flatten().cloned().collect_vec())
            })
            .collect_vec()
    }

    pub fn create_new_subgraph(
        &mut self,
        pair: Pair,
        path: Vec<SubGraphEdge>,
    ) -> Vec<PoolPairInfoDirection> {
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
        let subgraph = PairSubGraph::init(pair.ordered(), path);
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

    pub fn get_price(&self, pair: Pair) -> Option<Rational> {
        let (swapped, pair) = pair.ordered_changed();

        self.sub_graphs
            .get(&pair)
            .map(|graph| graph.fetch_price(&self.edge_state))
            .map(|res| if swapped { res.reciprocal() } else { res })
    }

    pub fn has_state(&self, addr: &Address) -> bool {
        self.edge_state.contains_key(addr)
    }
}

