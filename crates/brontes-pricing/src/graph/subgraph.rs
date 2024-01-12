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
use brontes_types::exchanges::StaticBindingsDb;
use itertools::Itertools;
use malachite::{
    num::basic::traits::{One, Zero},
    Rational,
};
use petgraph::{
    data::DataMap,
    graph::{self, DiGraph, UnGraph},
    prelude::*,
    visit::{
        Bfs, Data, GraphBase, IntoEdgeReferences, IntoEdges, IntoNeighbors, VisitMap, Visitable,
    },
    Graph,
};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use tracing::error;

use super::{PoolPairInfoDirection, PoolPairInformation};
use crate::{types::PoolState, Pair};

#[derive(Debug, Clone)]
pub struct SubGraphEdge {
    info: PoolPairInfoDirection,

    distance_to_token_0: usize,
    distance_to_token_1: usize,
}

/// PairSubGraph is a sub-graph that is made from the k-shortest paths for a
/// given Pair. This allows for running more complex search algorithms on the
/// graph and using weighted TVL to make sure that the calculated price is the
/// most correct.
#[derive(Debug, Clone)]
pub struct PairSubGraph {
    pair:  Pair,
    graph: DiGraph<(), Vec<SubGraphEdge>, usize>,

    start_node: usize,
    end_node:   usize,
}

impl PairSubGraph {
    pub fn init(pair: Pair, edges: Vec<SubGraphEdge>) -> Self {
        let mut graph = DiGraph::<(), Vec<SubGraphEdge>, usize>::default();

        let mut token_to_index = HashMap::new();

        let mut connections: HashMap<
            Address,
            (usize, HashMap<Address, (Vec<SubGraphEdge>, usize)>),
        > = HashMap::default();

        for edge in edges {
            let token_0 = edge.info.info.token_0;
            let token_1 = edge.info.info.token_1;

            // fetch the node or create node it if it doesn't exist
            let addr0 = *token_to_index
                .entry(token_0)
                .or_insert(graph.add_node(()).index());

            // fetch the node or create node it if it doesn't exist
            let addr1 = *token_to_index
                .entry(token_1)
                .or_insert(graph.add_node(()).index());

            // insert into connections
            if edge.info.token_0_in {
                let token_0_entry = connections
                    .entry(token_0)
                    .or_insert_with(|| (addr0, HashMap::default()));

                if let Some(inner) = token_0_entry.1.get_mut(&token_1) {
                    inner.0.push(edge);
                } else {
                    token_0_entry.1.insert(token_1, (vec![edge], addr1));
                }
            } else {
                let token_1_entry = connections
                    .entry(token_1)
                    .or_insert_with(|| (addr1, HashMap::default()));

                if let Some(inner) = token_1_entry.1.get_mut(&token_0) {
                    inner.0.push(edge);
                } else {
                    token_1_entry.1.insert(token_0, (vec![edge], addr0));
                }
            }
        }

        graph.extend_with_edges(connections.into_values().flat_map(|(node0, edges)| {
            edges
                .into_par_iter()
                .map(move |(_, (pools, adjacent))| {
                    (node0, adjacent, pools.into_iter().collect::<Vec<_>>())
                })
                .collect::<Vec<_>>()
        }));

        let start_node = token_to_index.remove(&pair.0).unwrap();
        let end_node = token_to_index.remove(&pair.1).unwrap();

        Self { pair, graph, start_node, end_node }
    }

    pub fn fetch_price(&self, edge_state: &HashMap<Address, PoolState>) -> Rational {
        dijkstra_path(&self.graph, self.start_node.into(), self.end_node.into(), edge_state)
            .expect("dijsktr on a subgraph failed, should be inpossible")
    }
}

// type EdgeRef: EdgeRef<NodeId = Self::NodeId, EdgeId = Self::EdgeId, Weight =
// Self::EdgeWeight>;
pub fn dijkstra_path<G>(
    graph: G,
    start: G::NodeId,
    goal: G::NodeId,
    state: &HashMap<Address, PoolState>,
) -> Option<Rational>
where
    // G::EdgeWeight = Vec<SubGraphEdge>,
    G: IntoEdgeReferences<EdgeWeight = Vec<SubGraphEdge>>,
    G: IntoEdges + Visitable,
    G::NodeId: Eq + Hash,
{
    let mut visited = graph.visit_map();
    let mut scores = HashMap::new();
    let mut predecessor = HashMap::new();
    let mut visit_next = BinaryHeap::new();
    let zero_score = Rational::ZERO;
    scores.insert(start, zero_score);
    visit_next.push(MinScored(zero_score, (start, Rational::ONE)));

    while let Some(MinScored(node_score, (node, price))) = visit_next.pop() {
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

            // given we have the previous price of the given node,
            // we can quote all tvl into the start asset by just keeping track of price.
            //
            // we sum up all the edges and get the total tvl plus
            let edge_weight = edge.weight();

            let (price_weight_sum, total_tvl) = edge_weight
                .iter()
                .map(|info| {
                    let pool_state = state.get(&info.info.info.pool_addr).unwrap();
                    let (t0, t1) = pool_state.get_tvl(info.info.get_base_token());



                    // (pool_state.get_price(info.info.get_base_token()), pool_state.get_tvl(info.info.get_base_token()))
                });
                // .fold((Rational::ONE, Rational::ZERO), |a, b| (a.0 + (b.0 * b.1), a.1 + b.1));

            let weighted_price_by_tvl = price_weight_sum * total_tvl;

            // Nodes that are more connected are given a shorter length. This
            // is done as we want to prioritize routing through a
            // commonly used token as the liquidity and pricing will
            // be more accurate than routing though a shit-coin. This will
            // also             // help as nodes with better connectivity will be searched
            // more than low             // connectivity nodes
            let next_score = node_score;
            let new_price = price;

            match scores.entry(next) {
                Occupied(ent) => {
                    if next_score < *ent.get() {
                        *ent.into_mut() = next_score;
                        visit_next.push(MinScored(next_score, (next, new_price)));
                        predecessor.insert(next, node);
                    }
                }
                Vacant(ent) => {
                    ent.insert(next_score);
                    visit_next.push(MinScored(next_score, (next, new_price)));
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

    // Some(path)
    None
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
