mod all_pair_graph;
mod registry;
mod subgraph;

use std::{
    cmp::{max, Ordering},
    collections::{
        hash_map::Entry::{Occupied, Vacant},
        BinaryHeap, HashMap, HashSet,
    },
    hash::Hash,
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
use tracing::{error, info};

use self::{all_pair_graph::AllPairGraph, registry::SubGraphRegistry, subgraph::SubGraphEdge};
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
    all_pair_graph: AllPairGraph,
    sub_graphs:     SubGraphRegistry,
}

impl GraphManager {
    pub fn init_from_db_state(
        all_pool_data: HashMap<(Address, StaticBindingsDb), Pair>,
        sub_graphs: HashMap<Pair, Vec<SubGraphEdge>>,
    ) -> Self {
        todo!()
    }

    pub fn add_node(&mut self, pair: Pair, pool_addr: Address, dex: StaticBindingsDb) {}

    /// creates a subpool for the pair returning all pools that need to be loaded
    pub fn create_subpool(&mut self, pair: Pair) -> Vec<Vec<PoolPairInfoDirection>> {
        todo!()
    }

    pub fn get_price(&self, pair: Pair) -> Option<Rational> {
        todo!()
    }

    pub fn new_state(&mut self, address: Address, state: PoolState) {

    }

    pub fn update_state(&mut self, state: PoolUpdate) {
        todo!()
    }

    pub fn has_state(&self, address: &Address) -> bool {
        false
    }
}
