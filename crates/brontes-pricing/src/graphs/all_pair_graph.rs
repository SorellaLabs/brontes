use std::{
    cmp::max,
    collections::{HashMap, HashSet},
    ops::{Deref, DerefMut},
    time::SystemTime,
};

use alloy_primitives::Address;
use brontes_types::pair::Pair;
use itertools::Itertools;
use petgraph::{graph::UnGraph, prelude::*};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use tracing::{error, info};

use super::yens::yen;
use crate::{PoolPairInfoDirection, PoolPairInformation, Protocol, SubGraphEdge};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EdgeWithInsertBlock {
    pub inner:        PoolPairInformation,
    pub insert_block: u64,
}

impl EdgeWithInsertBlock {
    pub fn new(
        pool_addr: Address,
        dex: Protocol,
        token0: Address,
        token1: Address,
        block_added: u64,
    ) -> Self {
        Self {
            inner:        PoolPairInformation::new(pool_addr, dex, token0, token1),
            insert_block: block_added,
        }
    }
}

impl Deref for EdgeWithInsertBlock {
    type Target = PoolPairInformation;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for EdgeWithInsertBlock {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}
/// [`AllPairGraph`] Represents the interconnected network of token pairs in
/// decentralized exchanges (DEXs), crucial for the BrontesBatchPricer system's
/// ability to analyze and calculate token prices.
///
/// [`AllPairGraph`] forms a graph structure where each node represents a token
/// and each edge a connection between tokens, typically through a liquidity
/// pool. This structure allows for efficient navigation and identification of
/// trading routes, and for determining the relative prices of tokens.
///
/// The graph is dynamic, adapting to the ever-changing landscape of the DEX
/// environment. It incorporates new tokens and pools as they emerge, and
/// adjusts or removes connections when changes in liquidity or pool validity
/// occur. This ensures that the representation of the token network remains
/// accurate and current.
///
/// The ability to assess the number of connections a token has, as well as to
/// identify paths for trading between any two tokens, is fundamental to the
/// system. It enables the evaluation of liquidity and trading opportunities.
/// The graph also provides the capability to exclude certain paths or
/// connections, catering to scenarios where specific routes might
/// be temporarily infeasible or less desirable.
#[derive(Debug, Clone)]
pub struct AllPairGraph {
    graph:          UnGraph<(), Vec<EdgeWithInsertBlock>, usize>,
    token_to_index: HashMap<Address, usize>,
}

impl AllPairGraph {
    pub fn init_from_hashmap(all_pool_data: HashMap<(Address, Protocol), Pair>) -> Self {
        let mut graph = UnGraph::<(), Vec<EdgeWithInsertBlock>, usize>::default();

        let mut token_to_index = HashMap::new();
        let mut connections: HashMap<(usize, usize), Vec<EdgeWithInsertBlock>> = HashMap::new();

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

            let info = EdgeWithInsertBlock::new(pool_addr, dex, pair.0, pair.1, 0);
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

    pub fn edge_count(&self, n0: Address, n1: Address) -> usize {
        let Some(n0) = self.token_to_index.get(&n0) else { return 0 };
        let Some(n1) = self.token_to_index.get(&n1) else { return 0 };
        let n0 = *n0;
        let n1 = *n1;

        let Some(edge) = self.graph.find_edge(n0.into(), n1.into()) else { return 0 };
        self.graph.edge_weight(edge).unwrap().len()
    }

    pub fn remove_empty_address(
        &mut self,
        pool_pair: Pair,
        pool_addr: Address,
    ) -> Option<(Address, Protocol, Pair)> {
        let Some(n0) = self.token_to_index.get(&pool_pair.0) else { return None };
        let Some(n1) = self.token_to_index.get(&pool_pair.1) else { return None };

        let Some(edge) = self.graph.find_edge((*n0).into(), (*n1).into()) else { return None };
        let Some(weights) = self.graph.edge_weight_mut(edge) else {
            return None;
        };

        let Some(bad_pool) = weights.iter().find(|e| e.pool_addr == pool_addr).cloned() else {
            return None;
        };
        weights.retain(|e| e.pool_addr != pool_addr);
        if weights.len() == 0 {
            self.graph.remove_edge(edge);
        }

        Some((bad_pool.pool_addr, bad_pool.dex_type, pool_pair))
    }

    pub fn add_node(&mut self, pair: Pair, pool_addr: Address, dex: Protocol, block: u64) {
        let pool_pair = EdgeWithInsertBlock::new(pool_addr, dex, pair.0, pair.1, block);

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
            let pair = vec![pool_pair];

            self.graph.add_edge(node_0.into(), node_1.into(), pair);
        }
    }

    pub(super) fn is_only_edge(&self, node: &Address) -> bool {
        let node = *self.token_to_index.get(node).unwrap();
        self.graph.edges(node.into()).collect_vec().len() == 1
    }

    pub(super) fn is_only_edge_ignoring(&self, node: &Address, ignore: &HashSet<Pair>) -> bool {
        let node = *self.token_to_index.get(node).unwrap();
        self.graph
            .edges(node.into())
            .filter(|edge| {
                let item = edge.weight().first().unwrap();
                let pair = Pair(item.token_0, item.token_1).ordered();
                !ignore.contains(&pair)
            })
            .collect_vec()
            .len()
            <= 1
    }

    pub fn get_paths_ignoring(
        &self,
        pair: Pair,
        ignore: &HashSet<Pair>,
        block: u64,
        connectivity_wight: usize,
    ) -> Vec<Vec<Vec<SubGraphEdge>>> {
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

        tracing::info!("yens with ignore list: {:#?}", ignore);
        yen(
            start_idx,
            |cur_node| {
                let cur_node: NodeIndex<usize> = (*cur_node).into();
                let edges = self.graph.edges(cur_node).collect_vec();
                let edge_len = edges.len() as isize;
                let weight = max(1, connectivity_wight as isize - edge_len);

                edges
                    .into_iter()
                    .filter(|f| {
                        if f.weight().iter().all(|e| e.insert_block > block) {
                            return false
                        }

                        f.weight()
                            .into_iter()
                            .map(|edge| {
                                let created_pair = Pair(edge.token_0, edge.token_1).ordered();
                                !ignore.contains(&created_pair)
                            })
                            .all(|a| a)
                    })
                    .filter(|e| !(e.source() == cur_node && e.target() == cur_node))
                    .map(|e| if e.source() == cur_node { e.target() } else { e.source() })
                    .map(|n| (n.index(), weight))
                    .collect_vec()
            },
            |node| node == end_idx,
            |node0, node1| (*node0, *node1),
            4,
            5_000,
        )
        .into_iter()
        .map(|(nodes, _)| {
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
                        .filter(|info| info.insert_block <= block)
                        .map(|info| {
                            let created_pair = Pair(info.token_0, info.token_1).ordered();
                            if ignore.contains(&created_pair) {
                                tracing::error!("ignore pair found in result");
                            }
                            let index = *self.token_to_index.get(&info.token_0).unwrap();
                            SubGraphEdge::new(
                                PoolPairInfoDirection {
                                    info:       *info,
                                    token_0_in: node0 == index,
                                },
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

    pub fn get_paths(
        &self,
        pair: Pair,
        block: u64,
        connectivity_wight: usize,
    ) -> Vec<Vec<Vec<SubGraphEdge>>> {
        let ignore = HashSet::new();
        self.get_paths_ignoring(pair, &ignore, block, connectivity_wight)
    }

    pub fn get_all_known_addresses(&self) -> Vec<Address> {
        self.token_to_index.keys().copied().collect_vec()
    }
}
