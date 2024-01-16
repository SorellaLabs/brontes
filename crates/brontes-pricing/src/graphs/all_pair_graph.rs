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

use super::{subgraph::SubGraphEdge, yens::yen, PoolPairInfoDirection, PoolPairInformation};

const CAPACITY: usize = 650_000;

/// All known pairs represented in the graph. All sub-graphs are generated off
/// of a k-shortest-path algorithm that is ran on this graph
#[derive(Debug, Clone)]
pub struct AllPairGraph {
    graph:          UnGraph<(), Vec<PoolPairInformation>, usize>,
    token_to_index: HashMap<Address, usize>,
}

impl AllPairGraph {
    pub fn init_from_hashmap(all_pool_data: HashMap<(Address, StaticBindingsDb), Pair>) -> Self {
        let mut graph =
            UnGraph::<(), Vec<PoolPairInformation>, usize>::with_capacity(CAPACITY / 2, CAPACITY);

        let mut token_to_index = HashMap::with_capacity(CAPACITY);
        let mut connections: HashMap<(usize, usize), Vec<PoolPairInformation>> = HashMap::new();

        let t0 = SystemTime::now();
        for ((pool_addr, dex), pair) in all_pool_data {
            // because this is undirected, doesn't matter what order the nodes are connected
            // so we sort so we can just have a collection of edges for just one
            // way
            let ordered_pair = pair.ordered();

            // fetch the node or create node it if it doesn't exist
            let addr0 = *token_to_index
                .entry(ordered_pair.0)
                .or_insert_with(|| graph.add_node(()).index());

            // fetch the node or create node it if it doesn't exist
            let addr1 = *token_to_index
                .entry(ordered_pair.1)
                .or_insert_with(|| graph.add_node(()).index());

            let info = PoolPairInformation::new(pool_addr, dex, pair.0, pair.1);
            connections.entry((addr0, addr1)).or_default().push(info);
        }

        let t1 = SystemTime::now();
        let delta = t1.duration_since(t0).unwrap().as_micros();
        info!("linked all graph edges in {}us", delta);
        let t0 = SystemTime::now();

        graph.extend_with_edges(
            connections
                .into_par_iter()
                .map(|((n0, n1), v)| (n0, n1, v))
                .collect::<Vec<_>>(),
        );

        let t1 = SystemTime::now();
        let delta = t1.duration_since(t0).unwrap().as_micros();

        info!(
            nodes=%graph.node_count(),
            edges=%graph.edge_count(),
            tokens=%token_to_index.len(),
            "built graph in {}us", delta
        );

        Self { graph, token_to_index }
    }

    pub fn add_node(&mut self, pair: Pair, pool_addr: Address, dex: StaticBindingsDb) {
        let pool_pair = PoolPairInformation::new(pool_addr, dex, pair.0, pair.1);

        let node_0 = *self
            .token_to_index
            .entry(pair.0)
            .or_insert(self.graph.add_node(()).index());

        let node_1 = *self
            .token_to_index
            .entry(pair.1)
            .or_insert(self.graph.add_node(()).index());

        if let Some(edge) = self.graph.find_edge(node_0.into(), node_1.into()) {
            let mut pools = self.graph.edge_weight(edge).unwrap().clone();
            pools.push(pool_pair);
            self.graph.update_edge(node_0.into(), node_1.into(), pools);
        } else {
            let mut pair = vec![pool_pair];

            self.graph.add_edge(node_0.into(), node_1.into(), pair);
        }
    }

    pub fn get_paths(&mut self, pair: Pair) -> Vec<Vec<Vec<SubGraphEdge>>> {
        if pair.0 == pair.1 {
            error!("Invalid pair, both tokens have the same address");
            return vec![]
        }

        let Some(start_idx) = self.token_to_index.get(&pair.0) else {
            let addr = pair.0;
            error!(?addr, "no node for address");
            return vec![]
        };
        let Some(end_idx) = self.token_to_index.get(&pair.1) else {
            let addr = pair.1;
            error!(?addr, "no node for address");
            return vec![]
        };

        yen(
            start_idx,
            |cur_node| {
                let cur_node: NodeIndex<usize> = (*cur_node).into();
                let edges = self.graph.edges(cur_node).collect_vec();
                let edge_len = edges.len() as isize;
                let weight = max(1, 1000_isize - edge_len);

                edges
                    .into_iter()
                    .filter(|e| !(e.source() == cur_node && e.target() == cur_node))
                    .map(|e| if e.source() == cur_node { e.target() } else { e.source() })
                    .map(|n| (n.index(), weight))
                    .collect_vec()
            },
            |node| node == end_idx,
            |node0, node1| (*node0, *node1),
            4,
        )
        .into_iter()
        .map(|(mut nodes, _)| {
            let path_length = nodes.len();
            nodes
                .into_iter()
                // default entry
                .filter(|(n0, n1)| n0 != n1)
                .enumerate()
                .map(|(i, (node0, node1))| {
                    self.graph
                        .edge_weight(
                            self.graph
                                .find_edge(node0.into(), node1.into())
                                .expect("no edge found"),
                        )
                        .unwrap()
                        .clone()
                        .into_iter()
                        .map(|info| {
                            let index = *self.token_to_index.get(&info.token_0).unwrap();
                            SubGraphEdge::new(
                                PoolPairInfoDirection { info, token_0_in: node0 == index },
                                i as u8,
                                (path_length - i) as u8,
                            )
                        })
                        .collect_vec()
                })
                .collect_vec()
        })
        .collect_vec()
    }

    pub fn get_all_known_addresses(&self) -> Vec<Address> {
        self.token_to_index.keys().copied().collect_vec()
    }
}
