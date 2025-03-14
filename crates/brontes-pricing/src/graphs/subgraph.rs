use std::{
    cmp::Ordering,
    collections::{
        hash_map::Entry::{Occupied, Vacant},
        BinaryHeap, VecDeque,
    },
    hash::Hash,
    sync::{
        atomic::{AtomicU64, Ordering::SeqCst},
        Arc,
    },
};

use alloy_primitives::Address;
use brontes_types::{price_graph_types::*, FastHashMap, FastHashSet};
use itertools::Itertools;
use malachite::{
    num::{
        arithmetic::traits::Reciprocal,
        basic::traits::{One, OneHalf, Zero},
    },
    Rational,
};
use petgraph::{
    algo::connected_components,
    graph::{EdgeReference, Edges},
    prelude::*,
    visit::{VisitMap, Visitable},
};
use tracing::error;

use crate::{types::ProtocolState, Pair};

pub struct VerificationOutcome {
    pub should_requery: bool,
    pub should_abandon: bool,
    pub removals:       FastHashMap<Pair, FastHashSet<BadEdge>>,
    pub frayed_ends:    Vec<Address>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Hash)]
pub struct BadEdge {
    pub pair:         Pair,
    pub pool_address: Address,
    // the edge of the pool that we calculated the liquidity for.
    pub edge_liq:     Address,
    pub liquidity:    Rational,
}

#[derive(Debug, Default)]
struct BfsArgs {
    pub removal_state: FastHashMap<Pair, FastHashSet<BadEdge>>,
}

const MIN_LIQUIDITY_USD_PEGGED_TOKEN: u128 = 15_000;
const MIN_LIQUIDITY_USD_PEGGED_TOKEN_RUNDOWN: u128 = 7_500;
const INACTIVE_REMOVAL_PERIOD: u64 = 750;

/// [`PairSubGraph`] is a directed subgraph, specifically designed to calculate
/// and optimize the pricing of a particular token pair in a decentralized
/// exchange environment. It extracts relevant paths from a larger token graph,
/// focusing on the most efficient paths between the pair of interest.
///
/// This struct is initialized with a specific token pair and their associated
/// edges, creating a directed graph where edges represent liquidity pools and
/// paths between tokens. The graph is tailored to efficiently compute the
/// best price for the given pair, leveraging algorithms that factor in the
/// total value locked and other relevant metrics.
///
/// The subgraph dynamically adapts to changes in the DEX, such as the
/// addition or removal of liquidity pools, to maintain accuracy in pricing.
/// It can identify and prune any unreliable or outdated information, such as
/// pools no longer active or offering sufficient liquidity.
///
/// The subgraph also plays a key role in verification processes, analyzing
/// and validating the state of the pools it comprises. This includes
/// ensuring the integrity and reliability of each pool's data within the
/// subgraph and recalculating prices based on up-to-date and verified
/// information.
#[derive(Debug, Clone)]
pub struct PairSubGraph {
    /// the pair represented
    pub(crate) pair:          Pair,
    pub(crate) complete_pair: Pair,
    /// the pair that trigged the need for pricing in the first place.
    must_go_through:          Pair,
    graph:                    DiGraph<(), Vec<SubGraphEdge>, u16>,
    token_to_index:           FastHashMap<Address, u16>,
    /// if this subgraph relies on another pair to calcuate the price.
    extends_to:               Option<Pair>,

    /// if a nodes liquidity drops more than 50% from when validation
    /// was last ran on this subgraph. a re_query is triggered.
    start_nodes_liq:        FastHashMap<Address, Rational>,
    start_node:             u16,
    end_node:               u16,
    /// the last time this subgraph was used for pricing.
    /// if it has been more than a certain amount of blocks, we will
    /// remove this graph to save on memory
    last_block_for_pricing: Arc<AtomicU64>,
    /// to avoid removing when there is a dependency
    /// we mark the removal time and all new subgraphs past this block,
    /// will generate a new subgrpah.
    remove_at:              Option<u64>,
}

impl PairSubGraph {
    pub fn init(
        pair: Pair,
        complete_pair: Pair,
        must_go_through: Pair,
        extends_to: Option<Pair>,
        edges: Vec<SubGraphEdge>,
        last_block_for_pricing: u64,
    ) -> Self {
        let mut graph = DiGraph::<(), Vec<SubGraphEdge>, u16>::default();
        let mut token_to_index = FastHashMap::default();

        let mut connections: FastHashMap<(u16, u16), Vec<SubGraphEdge>> = FastHashMap::default();
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
                .into_iter()
                .map(|((n0, n1), v)| (n0, n1, v))
                .collect::<Vec<_>>(),
        );

        let start_node = *token_to_index.get(&pair.0).unwrap();
        let end_node = *token_to_index.get(&pair.1).unwrap();

        let comp = connected_components(&graph);
        assert!(comp == 1, "have a disjoint graph {comp} {complete_pair:?}");

        Self {
            last_block_for_pricing: Arc::new(AtomicU64::new(last_block_for_pricing)),
            remove_at: None,
            pair,
            complete_pair,
            graph,
            start_node,
            end_node,
            token_to_index,
            extends_to,
            must_go_through,
            start_nodes_liq: FastHashMap::default(),
        }
    }

    pub fn should_use_for_new(&self) -> bool {
        self.remove_at.is_none()
    }

    pub fn ready_to_remove(&self, block: u64) -> bool {
        self.remove_at
            .map(|r_block| block > r_block)
            .unwrap_or(false)
    }

    pub fn complete_pair(&self) -> Pair {
        self.complete_pair
    }

    pub fn extends_to(&self) -> Option<Pair> {
        self.extends_to
    }

    pub fn must_go_through(&self) -> Pair {
        self.must_go_through
    }

    pub fn get_unordered_pair(&self) -> Pair {
        self.pair
    }

    pub fn save_last_verification_liquidity<T: ProtocolState>(
        &mut self,
        state: &FastHashMap<Address, &T>,
    ) {
        let init_tvl = self
            .graph
            .edge_weights()
            .flat_map(|weight| {
                weight.iter().filter_map(|edge| {
                    let (r0, r1) = state.get(&edge.pool_addr)?.tvl(edge.token_0);
                    let tvl_added = r0 + r1;

                    Some((edge.pool_addr, tvl_added))
                })
            })
            .collect::<FastHashMap<_, _>>();

        self.start_nodes_liq = init_tvl;
    }

    /// checks to see if the liquidity of any pool has dropped by over 50%.
    /// if this has happened, will send the pair for reverification
    pub fn has_stale_liquidity<T: ProtocolState>(&self, state: &FastHashMap<Address, &T>) -> bool {
        self.graph
            .edge_weights()
            .any(|weight| {
                weight
                    .iter()
                    .any(|edge| {
                        let (r0, r1) = state.get(&edge.pool_addr).unwrap().tvl(edge.token_0);
                        let tvl_added = r0 + r1;
                        let start_tvl = self.start_nodes_liq.get(&edge.pool_addr).unwrap();

                        if tvl_added < *start_tvl && start_tvl != &Rational::ZERO {
                            tvl_added / start_tvl <= Rational::ONE_HALF
                        } else {
                            false
                        }
                    })
            })
    }

    // returns list of pools we already have so we can derement there state tracker.
    pub fn extend_subgraph(&mut self, edges: Vec<SubGraphEdge>) -> Vec<Address> {
        let mut connections: FastHashMap<(u16, u16), Vec<SubGraphEdge>> = FastHashMap::default();
        let mut has = Vec::new();

        for edge in edges {
            let token_0 = edge.token_0;
            let token_1 = edge.token_1;

            // fetch the node or create node it if it doesn't exist
            let addr0 = *self
                .token_to_index
                .entry(token_0)
                .or_insert_with(|| self.graph.add_node(()).index().try_into().unwrap());

            // fetch the node or create node it if it doesn't exist
            let addr1 = *self
                .token_to_index
                .entry(token_1)
                .or_insert_with(|| self.graph.add_node(()).index().try_into().unwrap());

            // make sure is proper order
            let (addr0, addr1) = if edge.token_0_in { (addr0, addr1) } else { (addr1, addr0) };

            // check if we already have this edge so we don't add duplicates
            if let Some(g_edge) = self.graph.find_edge(addr0.into(), addr1.into()) {
                let edge_weight = self.graph.edge_weight_mut(g_edge).unwrap();
                if !edge_weight.contains(&edge) {
                    edge_weight.push(edge);
                }
                has.push(edge.pool_addr);
                continue
            }

            connections.entry((addr0, addr1)).or_default().push(edge);
        }

        self.graph.extend_with_edges(
            connections
                .into_iter()
                .map(|((n0, n1), v)| (n0, n1, v))
                .collect::<Vec<_>>(),
        );
        has
    }

    pub fn remove_bad_node(&mut self, pool_pair: Pair, pool_address: Address) -> bool {
        let Some(n0) = self.token_to_index.get(&pool_pair.0) else {
            tracing::debug!(?pool_address, "failed to remove bad edge");
            return false;
        };
        let Some(n1) = self.token_to_index.get(&pool_pair.1) else {
            tracing::debug!(?pool_address, "failed to remove bad edge");
            return false;
        };

        let n0 = (*n0).into();
        let n1 = (*n1).into();

        if let Some(edge) = self.graph.find_edge(n0, n1) {
            let weights = self.graph.edge_weight_mut(edge).unwrap();
            weights.retain(|e| e.pool_addr != pool_address);
            weights.is_empty()
        } else if let Some(edge) = self.graph.find_edge(n1, n0) {
            let weights = self.graph.edge_weight_mut(edge).unwrap();
            weights.retain(|e| e.pool_addr != pool_address);
            weights.is_empty()
        } else {
            tracing::debug!(
                ?pool_address,
                "failed to remove bad edge as no edge was found in the graph"
            );
            false
        }
    }

    pub fn fetch_price<T: ProtocolState>(
        &self,
        edge_state: &FastHashMap<Address, &T>,
    ) -> Option<Rational> {
        self.dijkstra_path(self.start_node.into(), edge_state)
    }

    pub fn get_all_pools(&self) -> impl Iterator<Item = &Vec<SubGraphEdge>> + '_ {
        self.graph.edge_weights()
    }

    pub fn add_new_edge(&mut self, edge_info: &'static PoolPairInformation) -> bool {
        let t0 = edge_info.token_0;
        let t1 = edge_info.token_1;

        // tokens have to already be in the graph for this edge to be added
        let Some(n0) = self.token_to_index.get(&t0) else {
            return false;
        };
        let Some(n1) = self.token_to_index.get(&t1) else {
            return false;
        };

        let node0 = (*n0).into();
        let node1 = (*n1).into();

        if let Some(edge) = self.graph.find_edge(node0, node1) {
            return add_edge(&mut self.graph, edge, edge_info, true)
        } else if let Some(edge) = self.graph.find_edge(node1, node0) {
            return add_edge(&mut self.graph, edge, edge_info, false)
        } else {
            let d0 = PoolPairInfoDirection { info: edge_info, token_0_in: true };
            let d1 = PoolPairInfoDirection { info: edge_info, token_0_in: false };

            let new_edge0 = SubGraphEdge::new(d0);
            let new_edge1 = SubGraphEdge::new(d1);

            self.graph.add_edge(node0, node1, vec![new_edge0]);
            self.graph.add_edge(node1, node0, vec![new_edge1]);
        }
        true
    }

    /// used to handle our memory management.
    pub fn is_expired_subgraph(&self, block: u64) -> bool {
        let last = self.last_block_for_pricing.load(SeqCst);
        if last > block {
            return false
        }
        (block - last) > INACTIVE_REMOVAL_PERIOD
    }

    pub fn future_use(&self, block: u64) {
        self.last_block_for_pricing.store(block, SeqCst);
    }

    pub fn has_valid_liquidity<T: ProtocolState>(
        &mut self,
        start: Address,
        start_price: Rational,
        state: &FastHashMap<Address, &T>,
        block: u64,
    ) {
        if self.remove_at.is_some() {
            return
        }

        let result = self.run_bfs_with_liquidity_params(start, start_price, state, true);

        // grab all edges below rundown threshold and remove. if disjoint, then
        // we abandon pricing for the given pair
        let edges = result
            .removal_state
            .into_iter()
            .flat_map(|(pair, v)| {
                v.into_iter()
                    .zip(vec![pair].into_iter().cycle())
                    .sorted_by(|a, b| a.0.liquidity.cmp(&b.0.liquidity))
            })
            .take_while(|(edge, _)| edge.liquidity <= MIN_LIQUIDITY_USD_PEGGED_TOKEN_RUNDOWN)
            .map(|(edge, pair)| (pair, edge.pool_address))
            .collect_vec();

        let pruned = self.tmp_prune(edges);
        let disjoint = self.is_disjoint();
        self.add_tmp_pruned(pruned);

        if disjoint {
            tracing::debug!(pair=?self.complete_pair, ?block, "invalid liquidity");
            self.remove_at = Some(block);
        }
    }

    pub fn rundown_subgraph_check<T: ProtocolState>(
        &mut self,
        start: Address,
        start_price: Rational,
        state: &FastHashMap<Address, &T>,
    ) -> VerificationOutcome {
        let result = self.run_bfs_with_liquidity_params(start, start_price, state, true);

        // grab all edges below rundown threshold and remove. if disjoint, then
        // we abandon pricing for the given pair
        let edges = result
            .removal_state
            .iter()
            .flat_map(|(pair, v)| {
                v.iter()
                    .zip(vec![pair].into_iter().cycle())
                    .sorted_by(|a, b| a.0.liquidity.cmp(&b.0.liquidity))
            })
            .take_while(|(edge, _)| edge.liquidity <= MIN_LIQUIDITY_USD_PEGGED_TOKEN_RUNDOWN)
            .map(|(edge, pair)| (*pair, edge.pool_address))
            .collect_vec();

        self.prune_subgraph_rundown(edges);

        let disjoint = self.is_disjoint();

        VerificationOutcome {
            should_requery: false,
            should_abandon: disjoint,
            removals:       result.removal_state,
            frayed_ends:    vec![],
        }
    }

    pub fn verify_subgraph<T: ProtocolState>(
        &mut self,
        start: Address,
        start_price: Rational,
        state: FastHashMap<Address, &T>,
    ) -> VerificationOutcome {
        tracing::debug!(?self.complete_pair, "verification starting");
        let result = self.run_bfs_with_liquidity_params(start, start_price, &state, false);

        tracing::debug!(?self.complete_pair, "completed bfs with liq");

        self.prune_subgraph(&result.removal_state);

        let disjoint = self.is_disjoint();

        tracing::debug!("disjoint: {disjoint}: bad: {}", result.removal_state.len());

        let frayed_ends = disjoint
            .then(|| self.disjoint_furthest_nodes())
            .unwrap_or_default();

        tracing::debug!(?self.complete_pair, "verification ending");
        // if we not disjoint, do a bad pool check.
        VerificationOutcome {
            should_requery: disjoint,
            should_abandon: false,
            removals: result.removal_state,
            frayed_ends,
        }
    }

    fn run_bfs_with_liquidity_params<T: ProtocolState>(
        &self,
        start: Address,
        start_price: Rational,
        state: &FastHashMap<Address, &T>,
        ignore_goes_through: bool,
    ) -> BfsArgs {
        self.bfs_with_price(
            start,
            start_price,
            |is_outgoing, edge, prev_price, removal_map: &mut BfsArgs| {
                let mut pxw = Rational::ZERO;
                let mut weight = Rational::ZERO;

                let node_weights = edge.weight();
                if node_weights.is_empty() {
                    return None
                }

                for info in node_weights {
                    let pair = Pair(info.token_0, info.token_1);

                    let Some(pool_state) = state.get(&info.pool_addr) else {
                        tracing::debug!(addr=?info.pool_addr,?pair, "failed to fetch pool state");
                        Self::bad_state(pair, info, Rational::ZERO, &mut removal_map.removal_state);
                        continue;
                    };

                    let Ok(pool_price) =
                        pool_state.price(info.get_token_with_direction(is_outgoing))
                    else {
                        Self::bad_state(pair, info, Rational::ZERO, &mut removal_map.removal_state);
                        continue;
                    };

                    let (t0, t1) = pool_state.tvl(info.get_token_with_direction(is_outgoing));
                    let liq0 = prev_price.clone().reciprocal() * &t0;

                    let goes_through_arg = if ignore_goes_through {
                        true
                    } else {
                        self.must_go_through != pair && self.must_go_through != pair.flip()
                    };
                    // check if below liquidity and that if we remove we don't make the graph
                    // disjoint.
                    if liq0 < MIN_LIQUIDITY_USD_PEGGED_TOKEN && goes_through_arg {
                        Self::bad_state(pair, info, liq0.clone(), &mut removal_map.removal_state);
                    } else {
                        let t0xt1 = &t0 * &t1;
                        pxw += pool_price * &t0xt1;
                        weight += t0xt1;
                    }
                }

                if weight == Rational::ZERO {
                    return None
                }

                let local_weighted_price = pxw / weight;

                Some(local_weighted_price)
            },
        )
    }

    fn bad_state(
        pair: Pair,
        info: &SubGraphEdge,
        liq: Rational,
        map: &mut FastHashMap<Pair, FastHashSet<BadEdge>>,
    ) {
        let bad_edge = BadEdge {
            pair,
            pool_address: info.pool_addr,
            edge_liq: info.get_quote_token(),
            liquidity: liq,
        };

        map.entry(pair).or_default().insert(bad_edge);
    }

    fn add_tmp_pruned(&mut self, data: Vec<(Pair, Vec<SubGraphEdge>, Direction)>) {
        data.into_iter()
            .for_each(|(k, bad_edge_to_pool, direction)| {
                let Some(n0) = self.token_to_index.get(&k.0) else {
                    tracing::error!("no token 0 in token to index");
                    return;
                };
                let Some(n1) = self.token_to_index.get(&k.1) else {
                    tracing::error!("no token 1 in token to index");
                    return;
                };
                let n0 = *n0;
                let n1 = *n1;

                if let Some((e, dir)) = self.graph.find_edge_undirected(n0.into(), n1.into()) {
                    let mut e = self.graph.remove_edge(e).unwrap();
                    e.extend(bad_edge_to_pool);
                    match dir {
                        Direction::Incoming => {
                            self.graph.add_edge(n1.into(), n0.into(), e);
                        }
                        Direction::Outgoing => {
                            self.graph.add_edge(n0.into(), n1.into(), e);
                        }
                    }
                } else {
                    match direction {
                        Direction::Incoming => {
                            self.graph
                                .update_edge(n1.into(), n0.into(), bad_edge_to_pool);
                        }
                        Direction::Outgoing => {
                            self.graph
                                .update_edge(n0.into(), n1.into(), bad_edge_to_pool);
                        }
                    }
                }
            });
    }

    fn tmp_prune(
        &mut self,
        data: Vec<(Pair, Address)>,
    ) -> Vec<(Pair, Vec<SubGraphEdge>, Direction)> {
        data.into_iter()
            .filter_map(|(k, bad_edge_to_pool)| {
                let Some(n0) = self.token_to_index.get(&k.0) else {
                    tracing::error!("no token 0 in token to index");
                    return None;
                };
                let Some(n1) = self.token_to_index.get(&k.1) else {
                    tracing::error!("no token 1 in token to index");
                    return None;
                };
                let n0 = *n0;
                let n1 = *n1;

                let Some((e, dir)) = self.graph.find_edge_undirected(n0.into(), n1.into()) else {
                    tracing::error!("no edge found");
                    return None;
                };

                let mut removed_weights = Vec::new();
                let mut weights = self.graph.remove_edge(e).unwrap();
                weights.retain(|node| {
                    let res = bad_edge_to_pool != node.pool_addr;
                    if !res {
                        removed_weights.push(*node);
                    }
                    res
                });

                if !weights.is_empty() {
                    match dir {
                        Direction::Incoming => {
                            self.graph.update_edge(n1.into(), n0.into(), weights);
                        }
                        Direction::Outgoing => {
                            self.graph.update_edge(n0.into(), n1.into(), weights);
                        }
                    }
                }

                Some((k, removed_weights, dir))
            })
            .collect()
    }

    fn prune_subgraph_rundown(&mut self, data: Vec<(Pair, Address)>) {
        data.into_iter().for_each(|(k, bad_edge_to_pool)| {
            let Some(n0) = self.token_to_index.get(&k.0) else {
                tracing::error!("no token 0 in token to index");
                return;
            };
            let Some(n1) = self.token_to_index.get(&k.1) else {
                tracing::error!("no token 1 in token to index");
                return;
            };
            let n0 = *n0;
            let n1 = *n1;

            let Some((e, dir)) = self.graph.find_edge_undirected(n0.into(), n1.into()) else {
                tracing::error!("no edge found");
                return;
            };

            let mut weights = self.graph.remove_edge(e).unwrap();
            weights.retain(|node| bad_edge_to_pool != node.pool_addr);

            if !weights.is_empty() {
                match dir {
                    Direction::Incoming => {
                        self.graph.update_edge(n1.into(), n0.into(), weights);
                    }
                    Direction::Outgoing => {
                        self.graph.update_edge(n0.into(), n1.into(), weights);
                    }
                }
            }
        })
    }

    fn prune_subgraph(&mut self, removal_state: &FastHashMap<Pair, FastHashSet<BadEdge>>) {
        removal_state.iter().for_each(|(k, v)| {
            let Some(n0) = self.token_to_index.get(&k.0) else {
                tracing::error!("no token 0 in token to index");
                return;
            };
            let Some(n1) = self.token_to_index.get(&k.1) else {
                tracing::error!("no token 1 in token to index");
                return;
            };
            let n0 = *n0;
            let n1 = *n1;

            let Some((e, dir)) = self.graph.find_edge_undirected(n0.into(), n1.into()) else {
                tracing::error!("no edge found");
                return;
            };

            let bad_edge_to_pool = v.iter().map(|edge| edge.pool_address).collect_vec();

            let mut weights = self.graph.remove_edge(e).unwrap();
            weights.retain(|node| !bad_edge_to_pool.contains(&node.pool_addr));
            if !weights.is_empty() {
                match dir {
                    Direction::Incoming => {
                        self.graph.update_edge(n1.into(), n0.into(), weights);
                    }
                    Direction::Outgoing => {
                        self.graph.update_edge(n0.into(), n1.into(), weights);
                    }
                }
            }
        });
    }

    fn next_edges_directed(
        &self,
        node: u16,
        outgoing: bool,
    ) -> Edges<'_, Vec<SubGraphEdge>, Directed, u16> {
        if outgoing {
            self.graph.edges_directed(node.into(), Direction::Outgoing)
        } else {
            self.graph.edges_directed(node.into(), Direction::Incoming)
        }
    }

    fn bfs_with_price<R: Default>(
        &self,
        start: Address,
        start_price: Rational,
        mut collect_data_fn: impl for<'a> FnMut(
            bool,
            EdgeReference<'a, Vec<SubGraphEdge>, u16>,
            &'a Rational,
            &'a mut R,
        ) -> Option<Rational>,
    ) -> R {
        let mut result = R::default();
        let mut visited = FastHashSet::default();
        let mut visit_next = VecDeque::new();

        let Some(start) = self.token_to_index.get(&start) else {
            error!(?start, ?start_price,?self.pair, ?self.complete_pair, ?self.must_go_through, ?self.extends_to, "no start node for bfs with price");
            return R::default();
        };

        let direction = *start == self.start_node;

        visit_next.extend(
            self.next_edges_directed(*start, direction)
                .zip(vec![start_price].into_iter().cycle()),
        );

        while let Some((next_edge, prev_price)) = visit_next.pop_front() {
            let id = next_edge.id();
            if visited.contains(&id) {
                continue
            }
            visited.insert(id);

            if let Some(price) = collect_data_fn(direction, next_edge, &prev_price, &mut result) {
                let new_price = &prev_price * price;

                let next_node = if direction { next_edge.target() } else { next_edge.source() };

                visit_next.extend(
                    self.next_edges_directed(next_node.index() as u16, direction)
                        .zip(vec![new_price].into_iter().cycle()),
                );
            }
        }

        result
    }

    /// given a dijsoint graph. finds the point at which the disjointness
    /// occurred.
    fn disjoint_furthest_nodes(&self) -> Vec<Address> {
        tracing::debug!(?self.pair, "grabing frayed ends");
        let mut frayed_ends = Vec::new();
        let mut visited = FastHashSet::default();
        let mut visit_next = VecDeque::new();

        visit_next.extend(
            self.graph
                .edges_directed(self.end_node.into(), Direction::Incoming),
        );

        while let Some(next_edge) = visit_next.pop_front() {
            let id = next_edge.id();
            if visited.contains(&id) {
                continue
            }
            visited.insert(id);

            let next_edges = self
                .graph
                .edges_directed(next_edge.source(), Direction::Incoming)
                .collect_vec();

            if next_edges.is_empty() {
                let node = next_edge.target().index() as u16;
                frayed_ends.push(
                    *self
                        .token_to_index
                        .iter()
                        .find(|(_, idx)| **idx == node)
                        .unwrap()
                        .0,
                );
                continue
            }
            visit_next.extend(next_edges);
        }
        tracing::debug!(?self.pair, "finished grabing frayed ends");

        frayed_ends
    }

    pub fn is_disjoint(&self) -> bool {
        let graph = &self.graph;

        let start: NodeIndex<u16> = self.start_node.into();
        let goal: NodeIndex<u16> = self.end_node.into();

        let mut visited = graph.visit_map();
        let mut scores = FastHashMap::default();
        let mut node_price = FastHashMap::default();
        let mut visit_next = BinaryHeap::new();
        let zero_score = Rational::ZERO;

        scores.insert(start, zero_score.clone());
        visit_next.push(MinScored(zero_score, start));

        while let Some(MinScored(node_score, node)) = visit_next.pop() {
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

                let next_score = &node_score + Rational::ONE;

                match scores.entry(next) {
                    Occupied(ent) => {
                        if next_score < *ent.get() {
                            *ent.into_mut() = next_score.clone();
                            visit_next.push(MinScored(next_score, next));
                            node_price.insert(next, ());
                        }
                    }
                    Vacant(ent) => {
                        ent.insert(next_score.clone());
                        visit_next.push(MinScored(next_score, next));
                        node_price.insert(next, ());
                    }
                }
            }
            visited.visit(node);
        }

        node_price.remove(&goal).is_none()
    }

    /// shows how well the given node is.
    pub fn first_hop_connections(&self) -> usize {
        let start: NodeIndex<u16> = self.start_node.into();
        self.graph
            .edges_directed(start, Direction::Outgoing)
            .flat_map(|f| f.weight())
            .count()
    }

    /// returns the pools liquidity in quote token for the other token that we
    /// are requesting price for. this allows us to have a good ref to how
    /// accurate the price is
    pub fn first_hop_min_liq<T: ProtocolState>(
        &self,
        state: &FastHashMap<Address, &T>,
    ) -> Option<Rational> {
        let start: NodeIndex<u16> = self.start_node.into();
        self.graph
            .edges_directed(start, Direction::Outgoing)
            .map(|f| (f.weight(), f.target()))
            .filter_map(|(pools, asset)| {
                let quote_price = self.dijkstra_path(asset, state)?;
                let mut min_liq = Rational::from(1_000_000_000u128);

                for pool in pools {
                    let Some(pool_e) = state.get(&pool.pool_addr) else { continue };
                    let (_, quote) = pool_e.tvl(pool.get_base_token());
                    if min_liq > quote {
                        min_liq = quote;
                    }
                }

                Some(quote_price * min_liq)
            })
            // dijkstra_path will take the most liquid path so we assume this will be taken
            .max()
    }

    pub fn dijkstra_path<T>(
        &self,
        start: NodeIndex<u16>,
        state: &FastHashMap<Address, &T>,
    ) -> Option<Rational>
    where
        T: ProtocolState,
    {
        let graph = &self.graph;
        let goal: NodeIndex<u16> = self.end_node.into();

        let mut visited = graph.visit_map();
        let mut scores = FastHashMap::default();
        let mut node_price = FastHashMap::default();
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
                let edge_weight = edge.weight();

                let next = edge.target();
                if visited.is_visited(&next) {
                    continue
                }

                let mut pxw = Rational::ZERO;
                let mut weight = Rational::ZERO;
                let mut token_0_am = Rational::ZERO;
                let mut token_1_am = Rational::ZERO;

                // calculate tvl of pool using the start token as the quote

                for info in edge_weight {
                    let Some(pool_state) = state.get(&info.pool_addr) else {
                        tracing::debug!(addr=?info.pool_addr,"failed to fetch pool state while generating price");
                        continue;
                    };

                    let Ok(pool_price) = pool_state.price(info.get_base_token()) else {
                        continue;
                    };

                    let (t0, t1) = pool_state.tvl(info.get_base_token());

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
                let token_0_priced = token_0_am * price.clone().reciprocal();
                let new_price = &price * local_weighted_price;
                let token_1_priced = token_1_am * new_price.clone().reciprocal();
                let tvl = token_0_priced + token_1_priced;
                let next_score = &node_score + std::cmp::max(Rational::ZERO, MAX_TVL_WEIGHT - tvl);

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
}

const MAX_TVL_WEIGHT: Rational = Rational::const_from_unsigned(100_000_000_000u64);

fn add_edge(
    graph: &mut DiGraph<(), Vec<SubGraphEdge>, u16>,
    edge_idx: EdgeIndex<u16>,
    edge_info: &'static PoolPairInformation,
    direction: bool,
) -> bool {
    let weights = graph.edge_weight_mut(edge_idx).unwrap();
    if weights
        .iter()
        .any(|w| w.pool_addr == { edge_info }.pool_addr)
    {
        return false
    }

    let new_edge =
        SubGraphEdge::new(PoolPairInfoDirection { info: edge_info, token_0_in: direction });
    weights.push(new_edge);

    true
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
    use brontes_types::Protocol;

    use super::*;

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
        fn price(&self, _base: Address) -> Result<Rational, crate::errors::ArithmeticError> {
            Ok(self.price.clone())
        }

        fn tvl(&self, _base: Address) -> (Rational, Rational) {
            self.tvl.clone()
        }
    }

    fn build_edge(lookup_pair: Address, t0: Address, t1: Address) -> SubGraphEdge {
        SubGraphEdge::new(PoolPairInfoDirection::new(
            Box::leak(Box::new(PoolPairInformation::new(lookup_pair, Protocol::UniswapV2, t0, t1))),
            true,
        ))
    }
    macro_rules! addresses {
        ($($var:ident),*) => {
            let mut bytes = [0u8; 20];
            $(
                let $var = Address::new(bytes);
                bytes[19] += 1;
            )*
        };
    }

    /// returns a graph that is just a straight line
    fn make_simple_graph() -> PairSubGraph {
        addresses!(t0, t1, t2, t3, t4);
        // t0 -> t1
        let e0 = build_edge(t0, t0, t1);
        // t1 -> t2
        let e1 = build_edge(t1, t1, t2);
        // t2 -> t3
        let e2 = build_edge(t2, t2, t3);
        // t3 -> t4
        let e3 = build_edge(t3, t3, t4);

        let pair = Pair(t0, t4);
        PairSubGraph::init(pair, pair, pair, None, vec![e0, e1, e2, e3], 0)
    }

    #[test]
    fn test_dijkstra_pricing() {
        addresses!(t0, t1, t2, t3, _t4);
        let graph = make_simple_graph();
        let mut state_map = FastHashMap::default();

        // t1 / t0 == 10
        let e0_price =
            MockPoolState::new(Rational::from(10), Rational::from(10_000), Rational::from(10_000));
        state_map.insert(t0, &e0_price);

        // t2 / t1 == 20
        let e1_price =
            MockPoolState::new(Rational::from(20), Rational::from(10_000), Rational::from(10_000));
        state_map.insert(t1, &e1_price);

        // t3 / t2 == 1 / 1500
        let e2_price = MockPoolState::new(
            Rational::from_unsigneds(1usize, 1500usize),
            Rational::from(10_000),
            Rational::from(10_000),
        );
        state_map.insert(t2, &e2_price);

        // t4 / t3 ==  1/52
        let e3_price = MockPoolState::new(
            Rational::from_unsigneds(1usize, 52usize),
            Rational::from(10_000),
            Rational::from(10_000),
        );
        state_map.insert(t3, &e3_price);

        // (t4 / t0) = 10 * 20 * 1 /500 * 1/52 = 1/130
        let price = graph.fetch_price(&state_map).unwrap();

        assert_eq!(price, Rational::from_unsigneds(1usize, 390usize))
    }
}
