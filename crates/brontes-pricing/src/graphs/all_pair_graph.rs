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
pub struct AllPairGraph {
    graph:          UnGraph<(), Vec<PoolPairInformation>, usize>,
    token_to_index: HashMap<Address, usize>,
}

impl AllPairGraph {
    pub fn init_from_hashmap(all_pool_data: HashMap<(Address, StaticBindingsDb), Pair>) -> Self {
        let mut graph =
            UnGraph::<(), Vec<PoolPairInformation>, usize>::with_capacity(CAPACITY / 2, CAPACITY);

        let mut token_to_index = HashMap::with_capacity(CAPACITY);

        let mut connections: HashMap<
            Address,
            (usize, HashMap<Address, (Vec<PoolPairInformation>, usize)>),
        > = HashMap::with_capacity(CAPACITY);

        let t0 = SystemTime::now();
        for ((pool_addr, dex), pair) in all_pool_data {
            // fetch the node or create node it if it doesn't exist
            let addr0 = *token_to_index
                .entry(pair.0)
                .or_insert(graph.add_node(()).index());

            // fetch the node or create node it if it doesn't exist
            let addr1 = *token_to_index
                .entry(pair.1)
                .or_insert(graph.add_node(()).index());

            // insert token0
            let token_0_entry = connections
                .entry(pair.0)
                .or_insert_with(|| (addr0, HashMap::default()));

            // if we find an already inserted edge, we append the address otherwise we
            // insert both
            if let Some(inner) = token_0_entry.1.get_mut(&pair.1) {
                inner
                    .0
                    .push(PoolPairInformation::new(pool_addr, dex, pair.0, pair.1));
            } else {
                token_0_entry.1.insert(
                    pair.1,
                    (vec![PoolPairInformation::new(pool_addr, dex, pair.0, pair.1)], addr1),
                );
            }

            // insert token1
            let token_1_entry = connections
                .entry(pair.1)
                .or_insert_with(|| (addr1, HashMap::default()));
            // if we find a already inserted edge, we append the address otherwise we insert
            // both
            if let Some(inner) = token_1_entry.1.get_mut(&pair.0) {
                inner
                    .0
                    .push(PoolPairInformation::new(pool_addr, dex, pair.0, pair.1));
            } else {
                token_1_entry.1.insert(
                    pair.0,
                    (vec![PoolPairInformation::new(pool_addr, dex, pair.0, pair.1)], addr0),
                );
            }
        }

        let t1 = SystemTime::now();
        let delta = t1.duration_since(t0).unwrap().as_micros();
        info!("linked all graph edges in {}us", delta);

        let t0 = SystemTime::now();
        graph.extend_with_edges(connections.into_values().flat_map(|(node0, edges)| {
            edges
                .into_par_iter()
                .map(move |(_, (pools, adjacent))| {
                    (node0, adjacent, pools.into_iter().collect::<Vec<_>>())
                })
                .collect::<Vec<_>>()
        }));

        let t1 = SystemTime::now();
        let delta = t1.duration_since(t0).unwrap().as_micros();

        info!(nodes=%graph.node_count(), edges=%graph.edge_count(), tokens=%token_to_index.len(), "built graph in {}us", delta);

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
                let weight = max(1, 100_isize - edge_len);

                edges
                    .into_iter()
                    .filter(|e| !(e.source() == cur_node && e.target() == cur_node))
                    .map(|e| if e.source() == cur_node { e.target() } else { e.source() })
                    .map(|n| (n.index(), edge_len))
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
                                PoolPairInfoDirection { info, token_0_in: node1 == index },
                                i,
                                path_length - i,
                            )
                        })
                        .collect_vec()
                })
                .collect_vec()
        })
        .collect_vec()
    }
}

fn insert_known_pair(entry: &mut Vec<Vec<PoolPairInfoDirection>>, pool: PoolPairInfoDirection) {
    if entry.is_empty() {
        entry.push(vec![pool]);
    } else {
        entry.get_mut(0).unwrap().push(pool);
    }
}
