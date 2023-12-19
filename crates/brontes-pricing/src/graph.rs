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
use brontes_types::{exchanges::StaticBindingsDb, extra_processing::Pair};
use itertools::Itertools;
use petgraph::{
    graph::UnGraph,
    prelude::*,
    visit::{IntoEdges, VisitMap, Visitable},
};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use tracing::info;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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
        Self { token_1, token_0, dex_type, pool_addr }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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

#[derive(Debug, Clone)]
pub struct PairGraph {
    graph:         UnGraph<(), HashSet<PoolPairInformation>, usize>,
    addr_to_index: HashMap<Address, usize>,
    // double vec for multi_hop multi_pool
    known_pairs:   HashMap<Pair, Vec<Vec<PoolPairInfoDirection>>>,
}

impl PairGraph {
    pub fn init_from_hashmap(map: HashMap<(Address, StaticBindingsDb), Pair>) -> Self {
        let t0 = SystemTime::now();
        let mut graph =
            UnGraph::<(), HashSet<PoolPairInformation>, usize>::with_capacity(500_000, 500_000);

        let mut addr_to_index = HashMap::default();
        let mut connections: HashMap<
            Address,
            (usize, Vec<(Address, Vec<PoolPairInformation>, usize)>),
        > = HashMap::new();

        let mut known_pairs: HashMap<Pair, Vec<Vec<PoolPairInfoDirection>>> = HashMap::new();

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

            // crate node if doesn't exist for addr or get node otherwise
            let addr0 = *addr_to_index
                .entry(pair.0)
                .or_insert(graph.add_node(()).index());

            // crate node if doesn't exist for addr or get node otherwise
            let addr1 = *addr_to_index
                .entry(pair.1)
                .or_insert(graph.add_node(()).index());

            // insert token0
            let e = connections.entry(pair.0).or_insert_with(|| (addr0, vec![]));

            // if we find a already inserted edge, we append the address otherwise we insert
            // both
            if let Some(inner) = e.1.iter_mut().find(|addr| addr.0 == pair.1) {
                inner
                    .1
                    .push(PoolPairInformation::new(pool_addr, dex, pair.0, pair.1));
            } else {
                e.1.push((
                    pair.1,
                    vec![PoolPairInformation::new(pool_addr, dex, pair.0, pair.1)],
                    addr1,
                ));
            }

            // insert token1
            let e = connections.entry(pair.1).or_insert_with(|| (addr1, vec![]));
            // if we find a already inserted edge, we append the address otherwise we insert
            // both
            if let Some(inner) = e.1.iter_mut().find(|addr| addr.0 == pair.0) {
                inner
                    .1
                    .push(PoolPairInformation::new(pool_addr, dex, pair.0, pair.1));
            } else {
                e.1.push((
                    pair.0,
                    vec![PoolPairInformation::new(pool_addr, dex, pair.0, pair.1)],
                    addr0,
                ));
            }
        }

        graph.extend_with_edges(connections.into_values().flat_map(|(node0, edges)| {
            edges
                .into_par_iter()
                .map(move |(_, pools, adjacent)| {
                    (node0, adjacent, pools.into_iter().collect::<HashSet<_>>())
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

        let direction0 = PoolPairInfoDirection { info: pool_pair.clone(), token_0_in: true };
        let direction1 = PoolPairInfoDirection { info: pool_pair.clone(), token_0_in: false };

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
            pools.insert(pool_pair);
            self.graph.update_edge(node_0.into(), node_1.into(), pools);
        } else {
            let mut set = HashSet::new();
            set.insert(pool_pair);

            self.graph.add_edge(node_0.into(), node_1.into(), set);
        }

        let t1 = SystemTime::now();
        let delta = t1.duration_since(t0).unwrap().as_micros();
        info!(delta, "added new node in us");
    }

    // fetches the path from start to end
    pub fn get_path(
        &mut self,
        pair: Pair,
    ) -> impl Iterator<Item = Vec<PoolPairInfoDirection>> + '_ {
        if let Some(pools) = self.known_pairs.get(&pair) {
            return pools.clone().into_iter()
        }

        let t0 = SystemTime::now();

        let Some(start_idx) = self.addr_to_index.get(&pair.0) else { return vec![].into_iter() };
        let Some(end_idx) = self.addr_to_index.get(&pair.1) else { return vec![].into_iter() };

        let path = dijkstra_path(&self.graph, (*start_idx).into(), (*end_idx).into())
            .expect("no path found, gotta make this into a option")
            .into_iter()
            .tuple_windows()
            .map(|(base, quote)| {
                self.graph
                    .edge_weight(self.graph.find_edge(base, quote).unwrap())
                    .unwrap()
                    .into_iter()
                    .map(|pool_info| {
                        let token_0_edge = *self.addr_to_index.get(&pool_info.token_0).unwrap();
                        if base.index() == token_0_edge {
                            PoolPairInfoDirection {
                                info:       pool_info.clone(),
                                token_0_in: true,
                            }
                        } else {
                            PoolPairInfoDirection {
                                info:       pool_info.clone(),
                                token_0_in: false,
                            }
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();

        self.known_pairs.insert(pair, path.clone());

        let t1 = SystemTime::now();
        let delta = t1.duration_since(t0).unwrap().as_micros();
        info!(%delta, pair=?pair, "found path to pair in us");

        path.into_iter()
    }
}

/// This modification to dijkstra weights the distance between nodes based of of
/// a max(0, 6 - connectivity). this is to favour better connected nodes as
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
        let connectivity = edges.len();

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
            let next_score = node_score + max(0, 6 - connectivity as isize);

            match scores.entry(next) {
                Occupied(ent) => {
                    if next_score < *ent.get() {
                        *ent.into_mut() = next_score;
                        visit_next.push(MinScored(next_score, next));
                        predecessor.insert(next.clone(), node.clone());
                    }
                }
                Vacant(ent) => {
                    ent.insert(next_score);
                    visit_next.push(MinScored(next_score, next));
                    predecessor.insert(next.clone(), node.clone());
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
