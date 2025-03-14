use std::{
    cmp::max,
    ops::Deref,
    time::{Duration, SystemTime},
};

use alloy_primitives::Address;
use brontes_types::{pair::Pair, FastHashMap, FastHashSet};
use itertools::Itertools;
use petgraph::prelude::*;
use tracing::{debug, error};

use super::yens::yen;
use crate::{LoadState, PoolPairInfoDirection, PoolPairInformation, Protocol, SubGraphEdge};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EdgeWithInsertBlock {
    pub inner:        &'static PoolPairInformation,
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
            inner:        Box::leak(Box::new(PoolPairInformation::new(
                pool_addr, dex, token0, token1,
            ))),
            insert_block: block_added,
        }
    }
}

impl Deref for EdgeWithInsertBlock {
    type Target = PoolPairInformation;

    fn deref(&self) -> &Self::Target {
        self.inner
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
    token_to_index: FastHashMap<Address, usize>,
}

impl AllPairGraph {
    pub fn init_from_hash_map(all_pool_data: FastHashMap<(Address, Protocol), Pair>) -> Self {
        let mut graph = UnGraph::<(), Vec<EdgeWithInsertBlock>, usize>::default();

        let mut token_to_index = FastHashMap::default();
        let mut connections: FastHashMap<(usize, usize), Vec<EdgeWithInsertBlock>> =
            FastHashMap::default();

        let t0 = SystemTime::now();

        all_pool_data
            .into_iter()
            .sorted()
            .for_each(|((pool_addr, dex), pair)| {
                if !dex.has_state_updater() {
                    return;
                }
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
            });

        let t1 = SystemTime::now();
        let delta = t1.duration_since(t0).unwrap().as_micros();
        debug!("linked all graph edges in {}us", delta);
        let t0 = SystemTime::now();

        graph.extend_with_edges(
            connections
                .into_iter()
                .sorted()
                .map(|((n0, n1), v)| (n0, n1, v))
                .collect::<Vec<_>>(),
        );

        let t1 = SystemTime::now();
        let delta = t1.duration_since(t0).unwrap().as_micros();

        debug!(
            nodes=%graph.node_count(),
            edges=%graph.edge_count(),
            tokens=%token_to_index.len(),
            "built graph in {}us", delta
        );

        Self { graph, token_to_index }
    }

    pub fn edge_count(&self, n0: Address, n1: Address) -> usize {
        let Some(n0) = self.token_to_index.get(&n0) else {
            return 0;
        };
        let Some(n1) = self.token_to_index.get(&n1) else {
            return 0;
        };
        let n0 = *n0;
        let n1 = *n1;

        let Some(edge) = self.graph.find_edge(n0.into(), n1.into()) else {
            return 0;
        };
        self.graph.edge_weight(edge).unwrap().len()
    }

    pub fn remove_empty_address(
        &mut self,
        pool_pair: Pair,
        pool_addr: Address,
    ) -> Option<(Address, Protocol, Pair)> {
        let n0 = self.token_to_index.get(&pool_pair.0)?;
        let n1 = self.token_to_index.get(&pool_pair.1)?;

        let edge = self.graph.find_edge((*n0).into(), (*n1).into())?;
        let weights = self.graph.edge_weight_mut(edge)?;
        let bad_pool = weights.iter().find(|e| e.pool_addr == pool_addr).cloned()?;
        weights.retain(|e| e.pool_addr != pool_addr);
        if weights.is_empty() {
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

    pub fn get_paths_ignoring(
        &self,
        pair: Pair,
        first_hop: Option<Pair>,
        ignore: &FastHashSet<Pair>,
        block: u64,
        connectivity_wight: usize,
        connections: Option<usize>,
        timeout: Duration,
        is_extension: bool,
        possible_extensions: Vec<Pair>,
    ) -> (Vec<Vec<Vec<SubGraphEdge>>>, Option<Pair>) {
        if pair.0 == pair.1 {
            error!("Invalid pair, both tokens have the same address");
            return (vec![], None);
        }

        let Some(start_idx) = first_hop
            .and_then(|fh| self.token_to_index.get(&fh.0))
            .or_else(|| self.token_to_index.get(&pair.0))
        else {
            let addr = pair.0;
            debug!(?addr, "no start node for address");
            return (vec![], None);
        };

        let second_idx = first_hop.and_then(|fh| self.token_to_index.get(&fh.1));

        let Some(end_idx) = self.token_to_index.get(&pair.1) else {
            let addr = pair.1;
            debug!(?addr, "no end node for address");
            return (vec![], None);
        };

        let mut indexes = possible_extensions
            .into_iter()
            .filter_map(|pair| Some((self.token_to_index.get(&pair.0).copied()?, pair)))
            .collect::<FastHashMap<_, _>>();

        let results = yen(
            start_idx,
            second_idx,
            |cur_node| {
                let cur_node: NodeIndex<usize> = (*cur_node).into();
                let edges = self.graph.edges(cur_node).collect_vec();
                let edge_len = edges.len() as isize;
                let weight = max(1, connectivity_wight as isize - edge_len);

                edges
                    .into_iter()
                    .filter(|f| {
                        if f.weight().iter().all(|e| e.insert_block > block) {
                            return false;
                        }

                        let edge = f.weight().first().unwrap();
                        let created_pair = Pair(edge.token_0, edge.token_1).ordered();
                        !ignore.contains(&created_pair)
                    })
                    .filter(|e| !(e.source() == cur_node && e.target() == cur_node))
                    .map(|e| if e.source() == cur_node { e.target() } else { e.source() })
                    .map(|n| (n.index(), weight))
                    .collect_vec()
            },
            |node| node == end_idx || indexes.contains_key(node),
            |node| node == end_idx,
            |node0, node1| (*node0, *node1),
            connections,
            7_500,
            timeout,
            is_extension,
            &indexes,
        )
        .into_iter()
        .map(|(nodes, _)| {
            nodes
                .into_iter()
                // default entry
                .filter(|(n0, n1)| n0 != n1)
                .map(|(node0, node1)| {
                    self.graph
                        .edge_weight(
                            self.graph
                                .find_edge(node0.into(), node1.into())
                                .expect("no edge found"),
                        )
                        .unwrap()
                        .iter()
                        .filter(|info| info.insert_block <= block)
                        .map(|info| {
                            let created_pair = Pair(info.token_0, info.token_1).ordered();
                            if ignore.contains(&created_pair) {
                                tracing::error!("ignore pair found in result");
                            }
                            let index = *self.token_to_index.get(&info.token_0).unwrap();
                            SubGraphEdge::new(PoolPairInfoDirection {
                                info:       info.inner,
                                token_0_in: node0 == index,
                            })
                        })
                        .collect_vec()
                })
                .collect_vec()
        })
        .collect_vec();

        let extends = results.last().and_then(|n| {
            n.last().and_then(|f| {
                f.last().and_then(|last| {
                    let token = if last.token_0_in { last.token_1 } else { last.token_0 };

                    let idx = self.token_to_index.get(&token).unwrap();
                    indexes.remove(idx)
                })
            })
        });

        (results, extends)
    }

    pub fn get_all_known_addresses(&self) -> Vec<Address> {
        self.token_to_index.keys().copied().collect_vec()
    }
}
