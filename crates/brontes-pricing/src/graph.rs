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
    algo::k_shortest_path,
    graph::{self, EdgeReference, UnGraph},
    prelude::*,
    visit::{Bfs, GraphBase, IntoEdges, IntoNeighbors, VisitMap, Visitable},
    Graph,
};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use serde::{Deserialize, Serialize};
use tracing::{error, info};

use crate::yens::yen;

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

const CAPACITY: usize = 650_000;

#[derive(Debug, Clone)]
pub struct PairGraph {
    //TODO: Try add address to nodes directly in the graph
    graph:         UnGraph<(), Vec<PoolPairInformation>, usize>,
    /// token address to node index in the graph
    addr_to_index: HashMap<Address, usize>,
    /// known pairs is a cache of pairs that have been requested before. This
    /// significatnly speeds up the graph as it reduces the amount of times
    /// that we search through the graph. it is set up to return all pools
    /// that can represent a swap leg e.g Curve WETH <> BTC UniV2 WETH <> BTC.
    /// This also supports the idea of a virtual pair. A virtual pair is a pair
    /// that can be represented by 2 or more sub pairs.
    known_pairs:   HashMap<Pair, Vec<Vec<PoolPairInfoDirection>>>,
}

impl PairGraph {
    pub fn init_from_hashmap(map: HashMap<(Address, StaticBindingsDb), Pair>) -> Self {
        let mut graph =
            UnGraph::<(), Vec<PoolPairInformation>, usize>::with_capacity(CAPACITY / 2, CAPACITY);

        let mut addr_to_index = HashMap::with_capacity(CAPACITY);

        let mut connections: HashMap<
            Address,
            (usize, HashMap<Address, (Vec<PoolPairInformation>, usize)>),
        > = HashMap::with_capacity(CAPACITY);

        let mut known_pairs: HashMap<Pair, Vec<Vec<PoolPairInfoDirection>>> =
            HashMap::with_capacity(CAPACITY);

        let t0 = SystemTime::now();
        for ((pool_addr, dex), pair) in map {
            // add the pool known in both directions
            let entry = known_pairs.entry(pair).or_default();
            insert_known_pair(
                entry,
                PoolPairInfoDirection {
                    info:       PoolPairInformation::new(pool_addr, dex, pair.0, pair.1),
                    token_0_in: true,
                },
            );

            let entry = known_pairs.entry(pair.flip()).or_default();
            insert_known_pair(
                entry,
                PoolPairInfoDirection {
                    info:       PoolPairInformation::new(pool_addr, dex, pair.0, pair.1),
                    token_0_in: false,
                },
            );

            // fetch the node or create node it if it doesn't exist
            let addr0 = *addr_to_index
                .entry(pair.0)
                .or_insert(graph.add_node(()).index());

            // fetch the node or create node it if it doesn't exist
            let addr1 = *addr_to_index
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

        info!(nodes=%graph.node_count(), edges=%graph.edge_count(), tokens=%addr_to_index.len(), "built graph in {}us", delta);

        Self { graph, addr_to_index, known_pairs }
    }

    pub fn add_node(&mut self, pair: Pair, pool_addr: Address, dex: StaticBindingsDb) {
        let t0 = SystemTime::now();
        let pool_pair = PoolPairInformation::new(pool_addr, dex, pair.0, pair.1);

        let direction0 = PoolPairInfoDirection { info: pool_pair, token_0_in: true };
        let direction1 = PoolPairInfoDirection { info: pool_pair, token_0_in: false };

        self.known_pairs.insert(pair, vec![vec![direction0]]);
        self.known_pairs.insert(pair.flip(), vec![vec![direction1]]);

        let node_0 = *self
            .addr_to_index
            .entry(pair.0)
            .or_insert(self.graph.add_node(()).index());

        let node_1 = *self
            .addr_to_index
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

        let t1 = SystemTime::now();
        let delta = t1.duration_since(t0).unwrap().as_micros();
        info!(us = delta, "added new node in");
    }

    /// fetches the path from start to end for the given pair inserting it into
    /// a hash map for quick lookups on additional queries.
    pub fn get_path(
        &mut self,
        pair: Pair,
    ) -> impl Iterator<Item = Vec<PoolPairInfoDirection>> + '_ {
        if pair.0 == pair.1 {
            error!("Invalid pair, both tokens have the same address");
            return vec![].into_iter()
        }

        if let Some(pools) = self.known_pairs.get(&pair) {
            return pools.clone().into_iter()
        }

        let Some(start_idx) = self.addr_to_index.get(&pair.0) else {
            let addr = pair.0;
            error!(?addr, "no node for address");
            return vec![].into_iter()
        };
        let Some(end_idx) = self.addr_to_index.get(&pair.1) else {
            let addr = pair.1;
            error!(?addr, "no node for address");
            return vec![].into_iter()
        };

        let path = dijkstra_path(&self.graph, (*start_idx).into(), (*end_idx).into())
            .unwrap_or_else(|| {
                error!(?pair, "couldn't find path between pairs");
                vec![]
            })
            .into_iter()
            .tuple_windows()
            .map(|(base, quote)| {
                self.graph
                    .edge_weight(self.graph.find_edge(base, quote).unwrap())
                    .unwrap()
                    .iter()
                    .map(|pool_info| {
                        let token_0_edge = *self.addr_to_index.get(&pool_info.token_0).unwrap();
                        if base.index() == token_0_edge {
                            PoolPairInfoDirection { info: *pool_info, token_0_in: true }
                        } else {
                            PoolPairInfoDirection { info: *pool_info, token_0_in: false }
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        self.known_pairs.insert(pair, path.clone());

        path.into_iter()
    }

    //TODO
    pub fn get_paths(&mut self, pair: Pair) -> Vec<Vec<Vec<PoolPairInfoDirection>>> {
        if pair.0 == pair.1 {
            error!("Invalid pair, both tokens have the same address");
            return vec![]
        }

        let Some(start_idx) = self.addr_to_index.get(&pair.0) else {
            let addr = pair.0;
            error!(?addr, "no node for address");
            return vec![]
        };
        let Some(end_idx) = self.addr_to_index.get(&pair.1) else {
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
                    .map(|e| if e.source() == cur_node { e.target() } else { e.source() })
                    .map(|n| (n.index(), edge_len))
                    .collect_vec()
            },
            |node| node == end_idx,
            |node0, node1| {
                self.graph
                    .edge_weight(
                        self.graph
                            .find_edge((*node0).into(), (*node1).into())
                            .unwrap(),
                    )
                    .unwrap()
                    .clone()
                    .into_iter()
                    .map(|info| {
                        let index = self.addr_to_index.get(&info.token_0).unwrap();
                        PoolPairInfoDirection { info, token_0_in: node0 == index }
                    })
                    .collect_vec()
            },
            1,
        )
        .into_iter()
        .map(|(i, _)| i)
        .collect_vec()
    }
}

#[cfg(feature = "benchmarking")]
impl PairGraph {
    pub fn get_all_known_addresses(&self) -> Vec<Address> {
        self.addr_to_index.keys().copied().collect()
    }

    /// fetches the path from start to end for the given pair inserting it into
    /// a hash map for quick lookups on additional queries.
    pub fn get_path_no_cache(&self, pair: Pair) {
        if pair.0 == pair.1 {
            return
        }

        let Some(start_idx) = self.addr_to_index.get(&pair.0) else {
            let addr = pair.0;
            error!(?addr, "no node for address");
            return
        };
        let Some(end_idx) = self.addr_to_index.get(&pair.1) else {
            let addr = pair.1;
            error!(?addr, "no node for address");
            return
        };

        let path = dijkstra_path(&self.graph, (*start_idx).into(), (*end_idx).into())
            .unwrap_or_else(|| {
                error!(?pair, "couldn't find path between pairs");
                vec![]
            })
            .into_iter()
            .tuple_windows()
            .map(|(base, quote)| {
                self.graph
                    .edge_weight(self.graph.find_edge(base, quote).unwrap())
                    .unwrap()
                    .iter()
                    .map(|pool_info| {
                        let token_0_edge = *self.addr_to_index.get(&pool_info.token_0).unwrap();
                        if base.index() == token_0_edge {
                            PoolPairInfoDirection { info: *pool_info, token_0_in: true }
                        } else {
                            PoolPairInfoDirection { info: *pool_info, token_0_in: false }
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
    }

    pub fn get_k_paths_no_cache(&mut self, pair: Pair) {
        if pair.0 == pair.1 {
            error!("Invalid pair, both tokens have the same address");
        }

        let Some(start_idx) = self.addr_to_index.get(&pair.0) else {
            let addr = pair.0;
            error!(?addr, "no node for address");
            return
        };
        let Some(end_idx) = self.addr_to_index.get(&pair.1) else {
            let addr = pair.1;
            error!(?addr, "no node for address");
            return
        };

        yen(
            start_idx,
            |cur_node| {
                let cur_node: NodeIndex<usize> = (*cur_node).into();
                let edges = self.graph.edges(cur_node).collect_vec();
                let edge_len = edges.len();

                edges
                    .into_iter()
                    .map(|e| if e.source() == cur_node { e.target() } else { e.source() })
                    .map(|n| (n.index(), edge_len))
                    .collect_vec()
            },
            |node| node == end_idx,
            |node0, node1| {
                self.graph
                    .edge_weight(
                        self.graph
                            .find_edge((*node0).into(), (*node1).into())
                            .unwrap(),
                    )
                    .unwrap()
                    .clone()
                    .into_iter()
                    .map(|info| {
                        let index = self.addr_to_index.get(&info.token_0).unwrap();
                        PoolPairInfoDirection { info, token_0_in: node0 == index }
                    })
                    .collect_vec()
            },
            4,
        )
        .into_iter()
        .map(|(i, _)| i)
        .collect_vec();
    }

    pub fn clear_pair_cache(&mut self) {
        self.known_pairs.clear();
    }
}

//TODO: Implement K simple shortest path algorithm to form subgraphs

/// This modification to dijkstra weights the distance between nodes based of of
/// a max(1, 20 - connectivity). this is to favour better connected nodes as
/// there price will be more accurate
pub fn dijkstra_path<G>(graph: G, start: G::NodeId, goal: G::NodeId) -> Option<Vec<G::NodeId>>
where
    G: IntoEdges + Visitable,
    G::NodeId: Eq + Hash,
{
    let mut visited = graph.visit_map();
    let mut scores = HashMap::new();
    let mut predecessor = HashMap::new();
    let mut visit_next = BinaryHeap::new();
    let zero_score = 0isize;
    scores.insert(start, zero_score);
    visit_next.push(MinScored(zero_score, start));
    while let Some(MinScored(node_score, node)) = visit_next.pop() {
        if visited.is_visited(&node) {
            continue
        }
        if goal == node {
            break
        }

        // grab the connectivity of the
        let edges = graph.edges(node).collect::<Vec<_>>();
        let connectivity = edges.len() as isize;

        for edge in graph.edges(node) {
            let next = edge.target();
            if visited.is_visited(&next) {
                continue
            }

            // Nodes that are more connected are given a shorter length. This
            // is done as we want to prioritize routing through a
            // commonly used token as the liquidity and pricing will
            // be more accurate than routing though a shit-coin. This will also
            // help as nodes with better connectivity will be searched more than low
            // connectivity nodes
            let next_score = node_score + max(1, 100 - connectivity);

            match scores.entry(next) {
                Occupied(ent) => {
                    if next_score < *ent.get() {
                        *ent.into_mut() = next_score;
                        visit_next.push(MinScored(next_score, next));
                        predecessor.insert(next, node);
                    }
                }
                Vacant(ent) => {
                    ent.insert(next_score);
                    visit_next.push(MinScored(next_score, next));
                    predecessor.insert(next, node);
                }
            }
        }
        visited.visit(node);
    }

    let mut path = Vec::new();

    let mut prev = predecessor.remove(&goal)?;
    path.push(goal);

    while let Some(next_prev) = predecessor.remove(&prev) {
        path.push(prev);
        prev = next_prev;
    }
    // add prev
    path.push(prev);
    // make start to finish
    path.reverse();

    Some(path)
}

fn insert_known_pair(entry: &mut Vec<Vec<PoolPairInfoDirection>>, pool: PoolPairInfoDirection) {
    if entry.is_empty() {
        entry.push(vec![pool]);
    } else {
        entry.get_mut(0).unwrap().push(pool);
    }
}
/// `MinScored<K, T>` holds a score `K` and a scored object `T` in
/// a pair for use with a `BinaryHeap`.
///
/// `MinScored` compares in reverse order by the score, so that we can
/// use `BinaryHeap` as a min-heap to extract the score-value pair with the
/// least score.
///
/// **Note:** `MinScored` implements a total order (`Ord`), so that it is
/// possible to use float types as scores.
#[derive(Copy, Clone, Debug)]
pub struct MinScored<K, T>(pub K, pub T);

impl<K: PartialOrd, T> PartialEq for MinScored<K, T> {
    #[inline]
    fn eq(&self, other: &MinScored<K, T>) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl<K: PartialOrd, T> Eq for MinScored<K, T> {}

impl<K: PartialOrd, T> PartialOrd for MinScored<K, T> {
    #[inline]
    fn partial_cmp(&self, other: &MinScored<K, T>) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<K: PartialOrd, T> Ord for MinScored<K, T> {
    #[inline]
    fn cmp(&self, other: &MinScored<K, T>) -> Ordering {
        let a = &self.0;
        let b = &other.0;
        if a == b {
            Ordering::Equal
        } else if a < b {
            Ordering::Greater
        } else if a > b {
            Ordering::Less
        } else if a.ne(a) && b.ne(b) {
            // these are the NaN cases
            Ordering::Equal
        } else if a.ne(a) {
            // Order NaN less, so that it is last in the MinScore order
            Ordering::Less
        } else {
            Ordering::Greater
        }
    }
}
