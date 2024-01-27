use std::{
    cmp::{max, Ordering},
    collections::{
        hash_map::Entry::{Occupied, Vacant},
        BinaryHeap, HashMap, HashSet, VecDeque,
    },
    hash::Hash,
    ops::{Deref, DerefMut},
    time::SystemTime,
};

use alloy_primitives::Address;
use itertools::Itertools;
use malachite::{
    num::{
        arithmetic::traits::{Abs, Reciprocal, ReciprocalAssign},
        basic::traits::{One, OneHalf, Zero},
    },
    Rational,
};
use petgraph::{
    algo::connected_components,
    data::{Build, DataMap, FromElements},
    graph::{self, DiGraph, Edges, UnGraph},
    prelude::*,
    stable_graph::IndexType,
    visit::{
        Bfs, Data, GraphBase, IntoEdgeReferences, IntoEdges, IntoEdgesDirected, IntoNeighbors,
        NodeRef, VisitMap, Visitable,
    },
    Graph,
};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use serde::{Deserialize, Serialize};
use tracing::{error, warn};

use crate::{
    price_graph_types::*,
    types::{PoolState, ProtocolState},
    AllPairGraph, Pair, Protocol,
};

const MIN_LIQUIDITY_USDC: u128 = 25_000;

/// PairSubGraph is a sub-graph that is made from the k-shortest paths for a
/// given Pair. This allows for running more complex search algorithms on the
/// graph and using weighted TVL to make sure that the calculated price is the
/// most correct.
#[derive(Debug, Clone)]
pub struct PairSubGraph {
    graph:          DiGraph<(), Vec<SubGraphEdge>, u16>,
    token_to_index: HashMap<Address, u16>,

    start_node: u16,
    end_node:   u16,
}

impl PairSubGraph {
    pub fn init(pair: Pair, edges: Vec<SubGraphEdge>) -> Self {
        let mut graph = DiGraph::<(), Vec<SubGraphEdge>, u16>::default();
        let mut token_to_index = HashMap::new();

        let mut connections: HashMap<(u16, u16), Vec<SubGraphEdge>> = HashMap::new();
        for edge in edges.into_iter() {
            let token_0 = edge.token_0;
            let token_1 = edge.token_1;

            // fetch the node or create node it if it doesn't exist
            let addr0 = *token_to_index
                .entry(token_0)
                .or_insert_with(|| graph.add_node(()).index().try_into().unwrap());

            // fetch the node or create node it if it doesn't exist
            let addr1 = *token_to_index
                .entry(token_1)
                .or_insert_with(|| graph.add_node(()).index().try_into().unwrap());

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

        Self { graph, start_node, end_node, token_to_index }
    }

    pub fn remove_bad_node(&mut self, pool_pair: Pair, pool_address: Address) -> bool {
        let Some(n0) = self.token_to_index.get(&pool_pair.0) else { return false };
        let Some(n1) = self.token_to_index.get(&pool_pair.1) else { return false };

        let n0 = (*n0).into();
        let n1 = (*n1).into();

        if let Some(edge) = self.graph.find_edge(n0, n1) {
            let weights = self.graph.edge_weight_mut(edge).unwrap();
            weights.retain(|e| e.pool_addr != pool_address);
            weights.len() == 0
        } else if let Some(edge) = self.graph.find_edge(n1, n0) {
            let weights = self.graph.edge_weight_mut(edge).unwrap();
            weights.retain(|e| e.pool_addr != pool_address);
            weights.len() == 0
        } else {
            false
        }
    }

    pub fn fetch_price<T: ProtocolState>(
        &self,
        edge_state: &HashMap<Address, T>,
    ) -> Option<Rational> {
        dijkstra_path(&self.graph, self.start_node.into(), self.end_node.into(), edge_state)
    }

    pub fn get_all_pools(&self) -> impl Iterator<Item = &Vec<SubGraphEdge>> + '_ {
        self.graph.edge_weights()
    }

    pub fn add_new_edge(&mut self, edge_info: PoolPairInformation) -> bool {
        let t0 = edge_info.token_0;
        let t1 = edge_info.token_1;

        // tokens have to already be in the graph for this edge to be added
        let Some(n0) = self.token_to_index.get(&t0) else { return false };
        let Some(n1) = self.token_to_index.get(&t1) else { return false };

        let node0 = (*n0).into();
        let node1 = (*n1).into();

        if let Some(edge) = self.graph.find_edge(node0, node1) {
            return add_edge(&mut self.graph, edge, edge_info, true)
        } else if let Some(edge) = self.graph.find_edge(node1, node0) {
            return add_edge(&mut self.graph, edge, edge_info, false)
        } else {
            // find the edge with shortest path
            let Some(to_start)= self
                .graph
                .edges(node0)
                .map(|e| e.weight().first().unwrap().distance_to_start_node)
                .min_by(|e0, e1| e0.cmp(e1)) else {
                    return false
                };

            let Some(to_end)= self
                .graph
                .edges(node1)
                .map(|e| e.weight().first().unwrap().distance_to_end_node)
                .min_by(|e0, e1| e0.cmp(e1)) else {
                    return false
                };

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

    pub fn verify_subgraph<T: ProtocolState>(
        &mut self,
        start: Address,
        state: &HashMap<Address, T>,
        all_pair_graph: &AllPairGraph,
    ) -> (bool, HashMap<Pair, Vec<Address>>) {
        let removal_state = self.run_bfs_with_liquidity_params(start, state, all_pair_graph);
        self.prune_subgraph(&removal_state);

        let disjoint =
            dijkstra_path(&self.graph, self.start_node.into(), self.end_node.into(), state)
                .is_none();

        (disjoint, removal_state)
    }

    fn run_bfs_with_liquidity_params<T: ProtocolState>(
        &self,
        start: Address,
        state: &HashMap<Address, T>,
        all_pair_graph: &AllPairGraph,
    ) -> HashMap<Pair, Vec<Address>> {
        self.bfs_with_price(
            start,
            |node_weights, prev_price, removal_map: &mut HashMap<Pair, Vec<Address>>| {
                let mut pxw = Rational::ZERO;
                let mut weight = Rational::ZERO;

                let mut possible_remove_pool_addr = Vec::new();
                let mut i = 0;

                for info in node_weights {
                    let Some(pool_state) = state.get(&info.pool_addr) else {
                        tracing::error!("no state");
                        continue;
                    };
                    // returns is t1  / t0
                    let Ok(pool_price) = pool_state.price(info.get_base_token()) else {
                        continue;
                    };
                    i += 1;

                    let (t0, t1) = pool_state.tvl(info.get_base_token());
                    let liq = prev_price.clone() * &t0;

                    // check if below liquidity and that if we remove we don't make the graph
                    // disjoint.
                    if liq < Rational::from(MIN_LIQUIDITY_USDC)
                        && !(all_pair_graph.is_only_edge(&info.token_0)
                            || all_pair_graph.is_only_edge(&info.token_1))
                    {
                        let pair = Pair(info.token_0, info.token_1).ordered();
                        removal_map.entry(pair).or_default().push(info.pool_addr);
                    } else {
                        let t0xt1 = &t0 * &t1;
                        pxw += pool_price * &t0xt1;
                        weight += t0xt1;
                    }

                    if liq < Rational::from(MIN_LIQUIDITY_USDC) {
                        possible_remove_pool_addr
                            .push((Pair(info.token_0, info.token_1).ordered(), info.pool_addr));
                    }
                }

                // check if we can remove some bad addresses in a edge. if we can,
                // then we do. and recalculate the price
                if possible_remove_pool_addr.len() < i {
                    possible_remove_pool_addr.iter().for_each(|(pair, addr)| {
                        removal_map.entry(pair.ordered()).or_default().push(*addr);
                    });
                }

                if weight == Rational::ZERO {
                    tracing::error!("no weight");
                    // means no edges were over limit, return
                    return None
                }

                let local_weighted_price = pxw / weight;

                Some(local_weighted_price)
            },
        )
    }

    fn prune_subgraph(&mut self, removal_state: &HashMap<Pair, Vec<Address>>) {
        removal_state.into_iter().for_each(|(k, v)| {
            let Some(n0) = self.token_to_index.get(&k.0) else { return };
            let Some(n1) = self.token_to_index.get(&k.1) else { return };
            let n0 = *n0;
            let n1 = *n1;

            let Some((e, dir)) = self.graph.find_edge_undirected(n0.into(), n1.into()) else {
                return
            };

            let mut weights = self.graph.remove_edge(e).unwrap();
            weights.retain(|node| !v.contains(&node.pool_addr));
            if !weights.is_empty() {
                match dir {
                    Direction::Incoming => {
                        self.graph.add_edge(n1.into(), n0.into(), weights);
                    }
                    Direction::Outgoing => {
                        self.graph.add_edge(n0.into(), n1.into(), weights);
                    }
                }
            }
        });
    }

    fn next_edges_directed<'a>(
        &'a self,
        node: u16,
        outgoing: bool,
    ) -> Edges<'a, Vec<SubGraphEdge>, Directed, u16> {
        if outgoing {
            self.graph.edges_directed(node.into(), Direction::Outgoing)
        } else {
            self.graph.edges_directed(node.into(), Direction::Incoming)
        }
    }

    fn bfs_with_price<R: Default>(
        &self,
        start: Address,
        mut collect_data_fn: impl for<'a> FnMut(
            &'a Vec<SubGraphEdge>,
            &'a Rational,
            &'a mut R,
        ) -> Option<Rational>,
    ) -> R {
        let mut result = R::default();
        let mut visited = HashSet::new();
        let mut visit_next = VecDeque::new();

        let Some(start) = self.token_to_index.get(&start) else { return R::default() };

        let direction = *start == self.start_node;

        visit_next.extend(
            self.next_edges_directed(*start, direction)
                .zip(vec![Rational::ONE].into_iter().cycle()),
        );

        while let Some((next_edge, prev_price)) = visit_next.pop_front() {
            let id = next_edge.id();
            if visited.contains(&id) {
                continue
            }
            visited.insert(id);

            if let Some(price) = collect_data_fn(next_edge.weight(), &prev_price, &mut result) {
                let new_price = &prev_price * price;
                let next_node = next_edge.target();
                visit_next.extend(
                    self.next_edges_directed(next_node.index() as u16, direction)
                        .zip(vec![new_price].into_iter().cycle()),
                );
            }
        }

        result
    }
}

fn add_edge(
    graph: &mut DiGraph<(), Vec<SubGraphEdge>, u16>,
    edge_idx: EdgeIndex<u16>,
    edge_info: PoolPairInformation,
    direction: bool,
) -> bool {
    let weights = graph.edge_weight_mut(edge_idx).unwrap();
    if weights
        .iter()
        .find(|w| w.pool_addr == edge_info.pool_addr)
        .is_some()
    {
        return false
    }

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
            let edge_len = edge_weight.len();

            let mut outliers = Vec::with_capacity(edge_len);
            let mut outlier_p = Rational::ZERO;
            let mut not_outliers = Vec::with_capacity(edge_len);
            let mut not_outlier_p = Rational::ZERO;

            for info in edge_weight {
                let Some(pool_state) = state.get(&info.pool_addr) else {
                    continue;
                };

                // returns is t1  / t0
                let Ok(pool_price) = pool_state.price(info.get_base_token()) else {
                    continue;
                };

                //  hacky method of splitting outliers from each_other. this assumes
                //  that the outliers fit into two distinct sets.
                //  for each entry, check the average price of the first set and if it is to far
                //  away put into other set. then after all state has been gone through. take
                // the longer set
                if not_outlier_p == Rational::ZERO {
                    not_outlier_p = pool_price.clone();
                    not_outliers.push((pool_price, pool_state.tvl(info.get_base_token())));
                } else if ((&not_outlier_p / Rational::from(not_outliers.len())) - &pool_price)
                    .abs()
                    > (&not_outlier_p / Rational::from(not_outliers.len())) / Rational::from(4)
                {
                    if outlier_p == Rational::ZERO {
                        outlier_p = pool_price.clone();
                        outliers.push((pool_price, pool_state.tvl(info.get_base_token())));
                    } else {
                        outlier_p += pool_price.clone();
                        outliers.push((pool_price, pool_state.tvl(info.get_base_token())));
                    }
                } else {
                    not_outlier_p += pool_price.clone();
                    not_outliers.push((pool_price, pool_state.tvl(info.get_base_token())));
                }
            }

            if not_outliers.len() == 0 && outliers.len() == 0 {
                continue
            }

            let set = if not_outliers.len() >= outliers.len() {
                not_outliers
            } else {
                let out_price = outliers.iter().map(|(i, _)| i).collect::<Vec<_>>();
                let min = out_price.iter().min().unwrap();
                let max = out_price.iter().max().unwrap();
                // more than 50% diff we take not outliers
                if *max / *min > Rational::ONE_HALF {
                    not_outliers
                } else {
                    outliers
                }
            };

            let mut pxw = Rational::ZERO;
            let mut weight = Rational::ZERO;
            let mut token_0_am = Rational::ZERO;
            let mut token_1_am = Rational::ZERO;

            for (pool_price, (t0, t1)) in set {
                // we only weight by the first token
                let t0xt1 = &t0 * &t1;
                pxw += pool_price * &t0xt1;
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
    use alloy_primitives::{hex, Address};
    use brontes_types::constants::USDC_ADDRESS;
    use futures::StreamExt;
    use serial_test::serial;

    use super::*;
    use crate::test_utils::PricingTestUtils;

    #[derive(Debug)]
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

    fn build_edge(lookup_pair: Address, t0: Address, t1: Address, d0: u8, d1: u8) -> SubGraphEdge {
        SubGraphEdge::new(
            PoolPairInfoDirection::new(
                PoolPairInformation::new(lookup_pair, Protocol::UniswapV2, t0, t1),
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
        let price = graph.fetch_price(&state_map).unwrap();

        assert_eq!(price, Rational::from_unsigneds(1usize, 390usize))
    }

    #[tokio::test]
    #[serial]
    async fn price_price_graph_for_shit() {
        let utils = PricingTestUtils::new(USDC_ADDRESS);
        let mut pricer = utils
            .setup_dex_pricer_for_tx(
                hex!("ebabf4a04fede867f7f681e30b4f5a79451e9d9e5bd1e50b4b455df8355571b6").into(),
            )
            .await
            .unwrap();
        pricer.next().await;
    }
}
