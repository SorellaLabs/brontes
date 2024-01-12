use std::collections::{HashMap, HashSet};

use alloy_primitives::Address;
use brontes_types::{exchanges::StaticBindingsDb, extra_processing::Pair};
use indexmap::set::Intersection;
use itertools::Itertools;
use malachite::Rational;

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
    pub fn new(
        cached_subgraphs: HashMap<Pair, PairSubGraph>,
        token_to_sub_graph: HashMap<Address, Vec<Pair>>,
    ) -> Self {
        todo!()
    }

    pub fn try_extend_subgraphs(
        &mut self,
        pool_address: Address,
        dex: StaticBindingsDb,
        pair: Pair,
    ) -> bool {
        let token_0 = pair.0;
        let token_1 = pair.1;

        let Some(t0_subgraph) = self.token_to_sub_graph.get(&token_0) else { return false };
        let Some(t1_subgraph) = self.token_to_sub_graph.get(&token_1) else { return false };
        let intersection = t0_subgraph
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
                subgraph.add_new_edge(info)
            })
            .collect_vec();

        return intersection.into_iter().any(|f| f)
    }

    pub fn create_new_subgraph(&mut self, pair: Pair, path: Vec<Vec<Vec<SubGraphEdge>>>) {
        // add to sub_graph lookup
        let tokens = path
            .iter()
            .flatten()
            .flatten()
            .flat_map(|i| [i.token_0, i.token_1])
            .collect::<HashSet<_>>();

        tokens.into_iter().for_each(|token| {
            self.token_to_sub_graph
                .entry(token)
                .or_default()
                .insert(pair);
        });

        let subgraph = PairSubGraph::init(pair, path);
        self.sub_graphs.insert(pair, subgraph);
    }

    pub fn update_pool_state(&mut self, pool_address: Address, update: PoolUpdate) -> Option<()> {
        Some(
            self.edge_state
                .get_mut(&pool_address)?
                .increment_state(update),
        )
    }

    pub fn get_price(&self, pair: Pair) -> Option<Rational> {
        self.sub_graphs
            .get(&pair)
            .map(|graph| graph.fetch_price(&self.edge_state))
    }
}
