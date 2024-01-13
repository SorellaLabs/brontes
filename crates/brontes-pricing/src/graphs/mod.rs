mod all_pair_graph;
mod dijkstras;
mod registry;
mod subgraph;
mod yens;
use std::{
    cmp::{max, Ordering},
    collections::{
        hash_map::Entry::{Occupied, Vacant},
        BinaryHeap, HashMap, HashSet,
    },
    hash::Hash,
    ops::{Deref, DerefMut},
    time::SystemTime,
};

use alloy_primitives::Address;
use brontes_types::{exchanges::StaticBindingsDb, extra_processing::Pair, tree::Node};
use ethers::core::k256::sha2::digest::HashMarker;
use itertools::Itertools;
use malachite::Rational;
use petgraph::{
    data::DataMap,
    graph::{self, UnGraph},
    prelude::*,
    visit::{Bfs, GraphBase, IntoEdges, IntoNeighbors, VisitMap, Visitable},
    Graph,
};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use serde::{Deserialize, Serialize};
pub use subgraph::SubGraphEdge;
use tracing::{error, info};

use self::{all_pair_graph::AllPairGraph, registry::SubGraphRegistry};
use super::PoolUpdate;
use crate::types::PoolState;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PoolPairInformation {
    pub pool_addr: Address,
    pub dex_type:  StaticBindingsDb,
    pub token_0:   Address,
    pub token_1:   Address,
}

impl PoolPairInformation {
    fn new(
        pool_addr: Address,
        dex_type: StaticBindingsDb,
        token_0: Address,
        token_1: Address,
    ) -> Self {
        Self { pool_addr, dex_type, token_0, token_1 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PoolPairInfoDirection {
    pub info:       PoolPairInformation,
    pub token_0_in: bool,
}

impl Deref for PoolPairInfoDirection {
    type Target = PoolPairInformation;

    fn deref(&self) -> &Self::Target {
        &self.info
    }
}

impl DerefMut for PoolPairInfoDirection {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.info
    }
}

impl PoolPairInfoDirection {
    pub fn get_base_token(&self) -> Address {
        if self.token_0_in {
            self.info.token_0
        } else {
            self.info.token_1
        }
    }
}

pub struct GraphManager {
    all_pair_graph:     AllPairGraph,
    sub_graph_registry: SubGraphRegistry,
    /// this is degen but don't want to reorganize all types so that
    /// this struct can hold the db so these closures allow for access...
    db_load: Box<dyn FnMut(u64, Pair) -> Option<(Pair, Vec<SubGraphEdge>)> + Send + Sync>,
    db_save:            Box<dyn FnMut(u64, Pair, Vec<SubGraphEdge>) + Send + Sync>,
}

impl GraphManager {
    pub fn init_from_db_state(
        all_pool_data: HashMap<(Address, StaticBindingsDb), Pair>,
        sub_graph_registry: HashMap<Pair, Vec<SubGraphEdge>>,
        db_load: Box<dyn FnMut(u64, Pair) -> Option<(Pair, Vec<SubGraphEdge>)> + Send + Sync>,
        db_save: Box<dyn FnMut(u64, Pair, Vec<SubGraphEdge>) + Send + Sync>,
    ) -> Self {
        let graph = AllPairGraph::init_from_hashmap(all_pool_data);
        let registry = SubGraphRegistry::new(sub_graph_registry);

        Self { all_pair_graph: graph, sub_graph_registry: registry, db_load, db_save }
    }

    pub fn add_pool(&mut self, block: u64, pair: Pair, pool_addr: Address, dex: StaticBindingsDb) {
        self.all_pair_graph.add_node(pair, pool_addr, dex);
    }

    /// creates a subpool for the pair returning all pools that need to be
    /// loaded
    pub fn create_subpool(&mut self, block: u64, pair: Pair) -> Vec<PoolPairInfoDirection> {
        if self.sub_graph_registry.has_subpool(&pair) {
            info!(?pair, "already have subgraph");
            /// fetch all state to be loaded
            return self.sub_graph_registry.fetch_unloaded_state(&pair)
        } else if let Some((pair, edges)) = (&mut self.db_load)(block, pair) {
            info!(?pair, "loaded subgraph from db");
            return self.sub_graph_registry.create_new_subgraph(pair, edges)
        }

        let paths = self
            .all_pair_graph
            .get_paths(pair)
            .into_iter()
            .flatten()
            .flatten()
            .collect_vec();

        // search failed
        if paths.is_empty() {
            info!(?pair, "empty search path");
            return vec![]
        }

        info!(?pair, "creating subgraph");
        self.sub_graph_registry
            .create_new_subgraph(pair, paths.clone())
    }

    pub fn get_price(&self, pair: Pair) -> Option<Rational> {
        self.sub_graph_registry.get_price(pair)
    }

    pub fn new_state(&mut self, block: u64, address: Address, state: PoolState) {
        self.sub_graph_registry
            .new_pool_state(address, state)
            .into_iter()
            .for_each(|(pair, edges)| {
                (&mut self.db_save)(block, pair, edges);
            });
    }

    pub fn update_state(&mut self, address: Address, update: PoolUpdate) {
        self.sub_graph_registry.update_pool_state(address, update);
    }

    pub fn has_state(&self, addr: &Address) -> bool {
        self.sub_graph_registry.has_state(addr)
    }
}
