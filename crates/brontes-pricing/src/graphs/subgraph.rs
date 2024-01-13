use core::panic;
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
use brontes_types::exchanges::StaticBindingsDb;
use itertools::Itertools;
use malachite::{
    num::{
        arithmetic::traits::{Reciprocal, ReciprocalAssign},
        basic::traits::{One, Zero},
    },
    Rational,
};
use petgraph::{
    algo::connected_components,
    data::{Build, DataMap},
    graph::{self, DiGraph, UnGraph},
    prelude::*,
    visit::{
        Bfs, Data, GraphBase, IntoEdgeReferences, IntoEdges, IntoNeighbors, VisitMap, Visitable,
    },
    Graph,
};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use serde::{Deserialize, Serialize};
use tracing::{error, warn};

use super::{PoolPairInfoDirection, PoolPairInformation};
use crate::{
    types::{PoolState, ProtocolState},
    Pair,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubGraphEdge {
    pub info: PoolPairInfoDirection,

    distance_to_start_node: usize,
    distance_to_end_node:   usize,
}
impl Deref for SubGraphEdge {
    type Target = PoolPairInfoDirection;

    fn deref(&self) -> &Self::Target {
        &self.info
    }
}
impl DerefMut for SubGraphEdge {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.info
    }
}

impl SubGraphEdge {
    pub fn new(
        info: PoolPairInfoDirection,
        distance_to_start_node: usize,
        distance_to_end_node: usize,
    ) -> Self {
        Self { info, distance_to_end_node, distance_to_start_node }
    }
}

/// PairSubGraph is a sub-graph that is made from the k-shortest paths for a
/// given Pair. This allows for running more complex search algorithms on the
/// graph and using weighted TVL to make sure that the calculated price is the
/// most correct.
#[derive(Debug, Clone)]
pub struct PairSubGraph {
    pair:           Pair,
    graph:          DiGraph<(), Vec<SubGraphEdge>, usize>,
    token_to_index: HashMap<Address, usize>,

    start_node: usize,
    end_node:   usize,
}

impl PairSubGraph {
    pub fn init(pair: Pair, edges: Vec<SubGraphEdge>) -> Self {
        let mut graph = DiGraph::<(), Vec<SubGraphEdge>, usize>::default();
        let mut token_to_index = HashMap::new();

        let mut connections: HashMap<(usize, usize), Vec<SubGraphEdge>> = HashMap::new();
        for edge in edges.into_iter() {
            let token_0 = edge.token_0;
            let token_1 = edge.token_1;

            // fetch the node or create node it if it doesn't exist
            let addr0 = *token_to_index
                .entry(token_0)
                .or_insert_with(|| graph.add_node(()).index());

            // fetch the node or create node it if it doesn't exist
            let addr1 = *token_to_index
                .entry(token_1)
                .or_insert_with(|| graph.add_node(()).index());

            // based on the direction. insert properly
            if edge.token_0_in {
                connections.entry((addr0, addr1)).or_default().push(edge);
            } else {
                connections.entry((addr1, addr0)).or_default().push(edge);
            }
        }

        graph.extend_with_edges(
            connections
                .into_par_iter()
                .map(|((n0, n1), v)| (n0, n1, v))
                .collect::<Vec<_>>(),
        );

        let start_node = *token_to_index.get(&pair.0).unwrap();
        let end_node = *token_to_index.get(&pair.1).unwrap();

        let comp = connected_components(&graph);
        assert!(comp == 1, "have a disjoint graph {comp} {pair:?}");

        Self { pair, graph, start_node, end_node, token_to_index }
    }

    pub fn fetch_price<T: ProtocolState>(&self, edge_state: &HashMap<Address, T>) -> Rational {
        dijkstra_path(&self.graph, self.start_node.into(), self.end_node.into(), edge_state)
            .expect("dijkstra on a subgraph failed, should be impossible")
    }

    pub fn get_all_pools(&self) -> impl Iterator<Item = &Vec<SubGraphEdge>> + '_ {
        self.graph.edge_weights()
    }

    pub fn add_new_edge(&mut self, edge_info: PoolPairInformation) -> bool {
        let t0 = edge_info.token_0;
        let t1 = edge_info.token_1;

        // tokens have to already be in the graph for this edge to be added
        let node0 = (*self.token_to_index.get(&t0).unwrap()).into();
        let node1 = (*self.token_to_index.get(&t1).unwrap()).into();

        if let Some(edge) = self.graph.find_edge(node0, node1) {
            return add_edge(&mut self.graph, edge, edge_info, true)
        } else if let Some(edge) = self.graph.find_edge(node1, node0) {
            return add_edge(&mut self.graph, edge, edge_info, false)
        } else {
            // find the edge with shortest path
            let to_start = self
                .graph
                .edges(node0)
                .map(|e| e.weight().first().unwrap().distance_to_start_node)
                .min_by(|e0, e1| e0.cmp(e1))
                .unwrap();

            let to_end = self
                .graph
                .edges(node1)
                .map(|e| e.weight().first().unwrap().distance_to_end_node)
                .min_by(|e0, e1| e0.cmp(e1))
                .unwrap();

            if !(to_start <= 1 && to_end <= 1) {
                return false
            }

            let d0 = PoolPairInfoDirection { info: edge_info.clone(), token_0_in: true };
            let d1 = PoolPairInfoDirection { info: edge_info, token_0_in: false };

            let new_edge0 = SubGraphEdge::new(d0, to_start, to_end);
            let new_edge1 = SubGraphEdge::new(d1, to_start, to_end);

            self.graph.add_edge(node0, node1, vec![new_edge0]);
            self.graph.add_edge(node1, node0, vec![new_edge1]);
        }
        true
    }
}

fn add_edge(
    graph: &mut DiGraph<(), Vec<SubGraphEdge>, usize>,
    edge_idx: EdgeIndex<usize>,
    edge_info: PoolPairInformation,
    direction: bool,
) -> bool {
    let weights = graph.edge_weight_mut(edge_idx).unwrap();
    let first = weights.first().unwrap();

    let to_start = first.distance_to_start_node;
    let to_end = first.distance_to_end_node;

    if !(to_start <= 1 && to_end <= 1) {
        return false
    }

    let new_edge = SubGraphEdge::new(
        PoolPairInfoDirection { info: edge_info, token_0_in: direction },
        to_start,
        to_end,
    );
    weights.push(new_edge);

    true
}

pub fn dijkstra_path<G, T>(
    graph: G,
    start: G::NodeId,
    goal: G::NodeId,
    state: &HashMap<Address, T>,
) -> Option<Rational>
where
    T: ProtocolState,
    G: IntoEdgeReferences<EdgeWeight = Vec<SubGraphEdge>>,
    G: IntoEdges + Visitable,
    G::NodeId: Eq + Hash,
{
    let mut visited = graph.visit_map();
    let mut scores = HashMap::new();
    let mut node_price = HashMap::new();
    let mut visit_next = BinaryHeap::new();
    let zero_score = Rational::ZERO;
    scores.insert(start, zero_score.clone());
    visit_next.push(MinScored(zero_score, (start, Rational::ONE)));

    while let Some(MinScored(node_score, (node, price))) = visit_next.pop() {
        if visited.is_visited(&node) {
            continue
        }

        if goal == node {
            break
        }

        for edge in graph.edges(node) {
            let next = edge.target();
            if visited.is_visited(&next) {
                continue
            }

            // calculate tvl of pool using the start token as the quote
            let edge_weight = edge.weight();

            let mut pxw = Rational::ZERO;
            let mut weight = Rational::ZERO;
            let mut token_0_am = Rational::ZERO;
            let mut token_1_am = Rational::ZERO;

            for info in edge_weight {
                let Some(pool_state) = state.get(&info.pool_addr) else {
                    warn!("no pool state found");
                    continue;
                };
                // returns is t1  / t0
                let Ok(pool_price) = pool_state.price(info.get_base_token()) else {
                    continue;
                };
                let (t0, t1) = pool_state.tvl(info.get_base_token());

                // we only weight by the first token
                let t0xt1 = &t0 * &t1;
                pxw += (pool_price * &t0xt1);
                weight += t0xt1;

                token_0_am += t0;
                token_1_am += t1;
            }

            if weight == Rational::ZERO {
                continue
            }

            let local_weighted_price = pxw / weight;

            let token_0_priced = token_0_am * &price;
            let new_price = &price * local_weighted_price;
            let token_1_priced = token_1_am * &new_price;

            let tvl = token_0_priced + token_1_priced;
            let next_score = &node_score + tvl.reciprocal();

            match scores.entry(next) {
                Occupied(ent) => {
                    if next_score < *ent.get() {
                        *ent.into_mut() = next_score.clone();
                        visit_next.push(MinScored(next_score, (next, new_price.clone())));
                        node_price.insert(next, new_price);
                    }
                }
                Vacant(ent) => {
                    ent.insert(next_score.clone());
                    visit_next.push(MinScored(next_score, (next, new_price.clone())));
                    node_price.insert(next, new_price);
                }
            }
        }
        visited.visit(node);
    }

    node_price.remove(&goal)
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

#[cfg(test)]
pub mod test {
    use alloy_primitives::Address;

    use super::*;

    struct MockPoolState {
        // tvl scaled by tokens
        tvl:   (Rational, Rational),
        // price as token1 / token0 where token0 is the base
        price: Rational,
    }

    impl MockPoolState {
        pub fn new(price: Rational, token0_tvl: Rational, token1_tvl: Rational) -> Self {
            Self { price, tvl: (token0_tvl, token1_tvl) }
        }
    }

    impl ProtocolState for MockPoolState {
        fn price(&self, base: Address) -> Result<Rational, crate::errors::ArithmeticError> {
            Ok(self.price.clone())
        }

        fn tvl(&self, base: Address) -> (Rational, Rational) {
            self.tvl.clone()
        }
    }

    fn build_edge(
        lookup_pair: Address,
        t0: Address,
        t1: Address,
        d0: usize,
        d1: usize,
    ) -> SubGraphEdge {
        SubGraphEdge::new(
            PoolPairInfoDirection::new(
                PoolPairInformation::new(lookup_pair, StaticBindingsDb::UniswapV2, t0, t1),
                true,
            ),
            d0,
            d1,
        )
    }
    macro_rules! addresses {
        ($($var:ident),*) => {
            let mut bytes = [0u8; 20];
            $(
                let $var = Address::new(bytes);
                bytes[19] += 1;
            )*;
        };
    }

    /// returns a graph that is just a straight line
    fn make_simple_graph() -> PairSubGraph {
        addresses!(t0, t1, t2, t3, t4);
        // t0 -> t1
        let e0 = build_edge(t0, t0, t1, 0, 7);
        // t1 -> t2
        let e1 = build_edge(t1, t1, t2, 1, 6);
        // t2 -> t3
        let e2 = build_edge(t2, t2, t3, 2, 5);
        // t3 -> t4
        let e3 = build_edge(t3, t3, t4, 3, 4);

        let pair = Pair(t0, t4);
        PairSubGraph::init(pair, vec![e0, e1, e2, e3])
    }

    #[test]
    fn test_dijkstra_pricing() {
        addresses!(t0, t1, t2, t3, t4);
        let mut graph = make_simple_graph();
        let mut state_map = HashMap::new();

        // t1 / t0 == 10
        let e0_price =
            MockPoolState::new(Rational::from(10), Rational::from(10_000), Rational::from(10_000));
        state_map.insert(t0, e0_price);

        // t2 / t1 == 20
        let e1_price =
            MockPoolState::new(Rational::from(20), Rational::from(10_000), Rational::from(10_000));
        state_map.insert(t1, e1_price);

        // t3 / t2 == 1 / 1500
        let e2_price = MockPoolState::new(
            Rational::from_unsigneds(1usize, 1500usize),
            Rational::from(10_000),
            Rational::from(10_000),
        );
        state_map.insert(t2, e2_price);

        // t4 / t3 ==  1/52
        let e3_price = MockPoolState::new(
            Rational::from_unsigneds(1usize, 52usize),
            Rational::from(10_000),
            Rational::from(10_000),
        );
        state_map.insert(t3, e3_price);

        // (t4 / t0) = 10 * 20 * 1 /500 * 1/52 = 1/130
        let price = graph.fetch_price(&state_map);

        assert_eq!(price, Rational::from_unsigneds(1usize, 390usize))
    }
}
