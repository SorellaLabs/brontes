//! [`BrontesBatchPricer`] calculates and track the prices of tokens
//! on decentralized exchanges on a per-transaction basis. It builds and
//! maintains a main token graph which is used to derive smaller subgraphs used
//! to price tokens relative to a defined quote token.
//!
//! ## Core Functionality
//!
//! ### Subgraph Utilization
//! The system leverages subgraphs, which are smaller, focused graph structures
//! extracted from the larger token graph. Subgraphs are built when a classified
//! event occurs on a token. When this occurs a subgraph is made for the pair if
//! one doesn't already exist. This allows for fast computation of a tokens
//! price. These subgraphs constantly update with new blocks, updating their
//! nodes and edges to reflect new liquidity pools.  
//!
//! ### Graph Management
//! The system adds new pools to the token graph as they appear in new blocks,
//! ensuring that all valid trading paths are represented.
//!
//! ### Lazy Loading
//! New pools and their states are fetched as required

use brontes_types::{
    db::dex::PriceAt, execute_on, normalized_actions::pool::NormalizedPoolConfigUpdate,
};
mod graphs;
pub mod protocols;
mod subgraph_query;
pub mod types;
use std::{
    collections::{hash_map::Entry, VecDeque},
    sync::{
        atomic::{AtomicBool, Ordering::SeqCst},
        Arc,
    },
    task::{Context, Poll},
};

use alloy_primitives::Address;
pub use brontes_types::price_graph_types::{
    PoolPairInfoDirection, PoolPairInformation, SubGraphEdge, SubGraphsEntry,
};
use brontes_types::{
    db::{
        dex::{DexPrices, DexQuotes},
        traits::{DBWriter, LibmdbxReader},
    },
    pair::Pair,
    traits::TracingProvider,
    FastHashMap, FastHashSet,
};
use futures::{Stream, StreamExt};
pub use graphs::{
    AllPairGraph, GraphManager, StateTracker, SubGraphRegistry, SubgraphVerifier,
    VerificationResults,
};
use itertools::Itertools;
use malachite::{
    num::{arithmetic::traits::Abs, basic::traits::One},
    Rational,
};
use protocols::lazy::{LazyExchangeLoader, LazyResult, LoadResult};
pub use protocols::{Protocol, *};
use subgraph_query::*;
use tokio::sync::mpsc::UnboundedReceiver;
use tracing::{error, info};
use types::{DexPriceMsg, PoolUpdate};

use crate::types::PoolState;
/// max movement of price in the block before its considered invalid.
/// currently 20%
const MAX_BLOCK_MOVEMENT: Rational = Rational::const_from_unsigneds(2, 10);

/// # Brontes Batch Pricer
///
/// [`BrontesBatchPricer`] establishes a token graph with liquidity pools as
/// edges
///
/// ## Workflow
/// The system operates on a block-by-block basis as follows:
///
/// 1) Incorporates new pools into the token graph with each new highest block
/// from the update channel.
///
/// 2) Uses a lazy loader to fetch data for all newly involved pools.
///
/// 3) Collects and buffers state transitions of all pools.
///
/// 4) After completing lazy loading, applies state transitions sequentially,
/// updating the price in the state map.
///
/// 5) Processes and returns formatted data from the applied state transitions
/// before proceeding to the next block.
pub struct BrontesBatchPricer<T: TracingProvider, DB: DBWriter + LibmdbxReader> {
    quote_asset:     Address,
    current_block:   u64,
    completed_block: u64,
    finished:        Arc<AtomicBool>,
    /// receiver from classifier, classifier is ran sequentially to guarantee
    /// order
    update_rx:       UnboundedReceiver<DexPriceMsg>,
    /// holds the state transfers and state void overrides for the given block.
    /// it works by processing all state transitions for a block and
    /// allowing lazy loading to occur. Once lazy loading has occurred and there
    /// are no more events for the current block, all the state transitions
    /// are applied in order with the price at the transaction index being
    /// calculated and inserted into the results and returned.
    buffer:          StateBuffer,
    /// holds new graph nodes / edges that can be added at every given block.
    /// this is done to ensure any route from a base to our quote asset will
    /// only pass though valid created pools.
    new_graph_pairs: FastHashMap<Address, (Protocol, Pair)>,
    /// manages all graph related items
    graph_manager:   GraphManager<DB>,
    /// lazy loads dex pairs so we only fetch init state that is needed
    lazy_loader:     LazyExchangeLoader<T>,
    dex_quotes:      FastHashMap<u64, DexQuotes>,
    /// pairs that failed to be verified. we use this to avoid the fallback for
    /// transfers
    failed_pairs:    FastHashMap<u64, Vec<(Pair, Pair)>>,
    /// when we are pulling from the channel, because its not peekable we always
    /// pull out one more than we want. this acts as a cache for it
    overlap_update:  Option<PoolUpdate>,
    /// a queue of blocks that we should skip pricing for and just upkeep state
    skip_pricing:    VecDeque<u64>,
}

impl<T: TracingProvider, DB: DBWriter + LibmdbxReader> BrontesBatchPricer<T, DB> {
    pub fn new(
        finished: Arc<AtomicBool>,
        quote_asset: Address,
        graph_manager: GraphManager<DB>,
        update_rx: UnboundedReceiver<DexPriceMsg>,
        provider: Arc<T>,
        current_block: u64,
        new_graph_pairs: FastHashMap<Address, (Protocol, Pair)>,
    ) -> Self {
        Self {
            finished,
            failed_pairs: FastHashMap::default(),
            new_graph_pairs,
            quote_asset,
            buffer: StateBuffer::new(),
            update_rx,
            graph_manager,
            dex_quotes: FastHashMap::default(),
            lazy_loader: LazyExchangeLoader::new(provider),
            current_block,
            completed_block: current_block,
            overlap_update: None,
            skip_pricing: VecDeque::new(),
        }
    }

    pub fn current_block_processing(&self) -> u64 {
        self.completed_block
    }

    /// testing / benching utils
    pub fn completed_block(&mut self) -> &mut u64 {
        &mut self.completed_block
    }

    /// testing / benching utils
    pub fn snapshot_graph_state(&self) -> (SubGraphRegistry, SubgraphVerifier, StateTracker) {
        self.graph_manager.snapshot_state()
    }

    /// testing / benching utils
    pub fn set_state(
        &mut self,
        sub_graph_registry: SubGraphRegistry,
        verifier: SubgraphVerifier,
        state: StateTracker,
    ) {
        self.graph_manager
            .set_state(sub_graph_registry, verifier, state)
    }

    /// Handles pool updates for the BrontesBatchPricer system.
    ///
    /// This function processes a vector of `PoolUpdate` messages, updating the
    /// current block tracking and incorporating new pools into the graph
    /// manager. It filters updates to identify and add new pools, using
    /// details such as address, protocol, and pair. The function also
    /// manages state transitions and pools, buffering state changes by
    /// block number and adding subgraphs for new pools if they don't
    /// already exist in the graph manager.
    ///
    /// Essentially, it ensures the graph manager remains synchronized with the
    /// latest block data, maintaining the integrity and accuracy of
    /// the decentralized exchange pricing mechanism.
    fn on_pool_updates(&mut self, updates: Vec<PoolUpdate>) {
        if updates.is_empty() {
            return
        };

        if let Some(msg) = updates.first() {
            if msg.block > self.current_block {
                self.current_block = msg.block;
            }
        }

        // insert new pools accessed on this block.
        updates
            .iter()
            .filter_map(|update| {
                let (protocol, pair) = self.new_graph_pairs.remove(&update.get_pool_address())?;
                Some((update.get_pool_address(), protocol, pair, update.block))
            })
            .for_each(|(pool_addr, protocol, pair, block)| {
                self.graph_manager
                    .add_pool(pair, pool_addr, protocol, block);
            });

        tracing::debug!("search triggered by pool updates");
        let (state, pools) = execute_on!(target = pricing, {
            graph_search_par(&self.graph_manager, self.quote_asset, updates)
        });
        tracing::debug!("search triggered by on pool updates completed");

        state.into_iter().flatten().for_each(|(addr, update)| {
            let block = update.block;
            self.buffer
                .updates
                .entry(block)
                .or_default()
                .push_back((addr, update));
        });

        pools.into_iter().flatten().for_each(
            |NewGraphDetails { must_include, complete_pair, pair, extends_pair, block, edges }| {
                if edges.is_empty() {
                    tracing::debug!(
                        ?pair,
                        ?complete_pair,
                        ?must_include,
                        ?extends_pair,
                        "new pool has no graph edges"
                    );
                    return
                }

                if self.graph_manager.has_subgraph_goes_through(
                    pair,
                    (!must_include.is_zero()).then_some(must_include),
                ) {
                    tracing::debug!(?pair, "already have pairs");
                    return
                }

                self.add_subgraph(
                    pair,
                    complete_pair,
                    must_include,
                    extends_pair,
                    block,
                    edges,
                    false,
                );
            },
        );
    }

    fn on_pool_update_no_pricing(&mut self, updates: Vec<PoolUpdate>) {
        if let Some(msg) = updates.first() {
            if msg.block > self.current_block {
                self.current_block = msg.block;
                self.completed_block = msg.block + 1;
            }
        }

        updates
            .iter()
            .filter_map(|update| {
                let (protocol, pair) = self.new_graph_pairs.remove(&update.get_pool_address())?;
                Some((update.get_pool_address(), protocol, pair, update.block))
            })
            .for_each(|(pool_addr, protocol, pair, block)| {
                self.graph_manager
                    .add_pool(pair, pool_addr, protocol, block);
            });

        updates.into_iter().for_each(|update| {
            self.graph_manager
                .update_state(update.get_pool_address(), update);
        });
    }

    fn get_dex_price(
        &mut self,
        pool_pair: Pair,
        goes_through: Pair,
        goes_through_address: Option<Address>,
    ) -> Option<Rational> {
        if pool_pair.0 == pool_pair.1 {
            return Some(Rational::ONE)
        }
        self.graph_manager
            .get_price(pool_pair, goes_through, goes_through_address)
    }

    /// For a given block number and tx idx, finds the path to the following
    /// tokens and inserts the data into dex_quotes.
    fn store_dex_price(&mut self, block: u64, tx_idx: u64, pool_pair: Pair, prices: DexPrices) {
        // insert the pool keys into the price map
        match self.dex_quotes.entry(block) {
            Entry::Occupied(mut quotes) => {
                let q = quotes.get_mut();
                let size = q.0.len();
                // pad the vector
                for _ in size..=tx_idx as usize {
                    q.0.push(None)
                }
                let tx = q.0.get_mut(tx_idx as usize).unwrap();

                if let Some(tx) = tx.as_mut() {
                    tx.insert(pool_pair, prices);
                } else {
                    let mut tx_pairs = FastHashMap::default();
                    tx_pairs.insert(pool_pair, prices);
                    *tx = Some(tx_pairs)
                }
            }
            Entry::Vacant(v) => {
                // pad the vec to the tx index
                let mut vec = Vec::new();
                for _ in 0..=tx_idx as usize {
                    vec.push(None);
                }
                // insert
                let mut map = FastHashMap::default();
                map.insert(pool_pair, prices);

                let entry = vec.get_mut(tx_idx as usize).unwrap();
                *entry = Some(map);

                v.insert(DexQuotes(vec));
            }
        }
    }

    /// Similar to update known state but doesn't apply the state transfer given
    /// the pool is from end of block.
    fn init_new_pool_override(&mut self, addr: Address, msg: PoolUpdate) {
        let tx_idx = msg.tx_idx;
        let block = msg.block;
        let pool = msg.get_pool_address_for_pricing();

        let Some(pool_pair) = msg.get_pair(self.quote_asset) else {
            info!(?addr, "failed to get pair for pool");
            return;
        };

        // generate all variants of the price that might be used in the inspectors
        let pair0 = Pair(pool_pair.0, self.quote_asset);
        let pair1 = Pair(pool_pair.1, self.quote_asset);

        let flipped_pool = pool_pair.flip();

        if let Some(price0) = self.get_dex_price(pair0, pool_pair, pool) {
            let mut bad = false;
            self.failed_pairs.retain(|r_block, s| {
                if block != *r_block {
                    return true
                }
                s.retain(|(p, gt)| {
                    if *p == pair0 && *gt == pool_pair {
                        bad = true;
                        false
                    } else {
                        true
                    }
                });

                !s.is_empty()
            });

            if !bad {
                let price0 = DexPrices {
                    post_state:   price0.clone(),
                    pre_state:    price0,
                    goes_through: pool_pair,
                };
                self.store_dex_price(block, tx_idx, pair0, price0);
            }
        };

        if let Some(price1) = self.get_dex_price(pair1, flipped_pool, pool) {
            let mut bad = false;
            self.failed_pairs.retain(|r_block, s| {
                if block != *r_block {
                    return true
                }
                s.retain(|(p, gt)| {
                    if *p == pair1 && *gt == flipped_pool {
                        bad = true;
                        false
                    } else {
                        true
                    }
                });

                !s.is_empty()
            });

            if !bad {
                let price1 = DexPrices {
                    post_state:   price1.clone(),
                    pre_state:    price1,
                    goes_through: flipped_pool,
                };
                self.store_dex_price(block, tx_idx, pair1, price1);
            }
        };
    }

    fn update_known_state(&mut self, addr: Address, msg: PoolUpdate) {
        let tx_idx = msg.tx_idx;
        let block = msg.block;
        let pool = msg.get_pool_address_for_pricing();
        let Some(pool_pair) = msg.get_pair(self.quote_asset) else {
            error!(?addr, "failed to get pair for pool");
            self.graph_manager.update_state(addr, msg);
            return;
        };

        let pair0 = Pair(pool_pair.0, self.quote_asset);
        let pair1 = Pair(pool_pair.1, self.quote_asset);

        let flipped_pool = pool_pair.flip();

        let price0_pre = self.get_dex_price(pair0, pool_pair, pool);
        let price1_pre = self.get_dex_price(pair1, flipped_pool, pool);

        self.graph_manager.update_state(addr, msg);

        let price0_post = self.get_dex_price(pair0, pool_pair, pool);
        let price1_post = self.get_dex_price(pair1, flipped_pool, pool);

        if let (Some(price0_pre), Some(price0_post)) = (price0_pre, price0_post) {
            let mut bad = false;
            self.failed_pairs.retain(|r_block, s| {
                if block != *r_block {
                    return true
                }
                s.retain(|(p, gt)| {
                    if *p == pair0 && *gt == pool_pair {
                        bad = true;
                        false
                    } else {
                        true
                    }
                });

                !s.is_empty()
            });

            if !bad {
                self.store_dex_price(
                    block,
                    tx_idx,
                    pair0,
                    DexPrices {
                        pre_state:    price0_pre,
                        post_state:   price0_post,
                        goes_through: pool_pair,
                    },
                );
            }
        }

        if let (Some(price1_pre), Some(price1_post)) = (price1_pre, price1_post) {
            let mut bad = false;
            self.failed_pairs.retain(|r_block, s| {
                if block != *r_block {
                    return true
                }
                s.retain(|(p, gt)| {
                    if *p == pair1 && *gt == flipped_pool {
                        bad = true;
                        false
                    } else {
                        true
                    }
                });

                !s.is_empty()
            });
            if !bad {
                self.store_dex_price(
                    block,
                    tx_idx,
                    pair1,
                    DexPrices {
                        pre_state:    price1_pre,
                        post_state:   price1_post,
                        goes_through: flipped_pool,
                    },
                );
            }
        }
    }

    /// Processes the result of lazy pool state loading.
    ///
    /// Updates the graph state or handles errors.
    ///
    /// # Behavior
    /// If the pool state is successfully loaded, the function updates the graph
    /// manager with the new state.
    ///
    /// If the pool was initialized in the current block and the load result
    /// indicates an error, an override is set to prevent invalid state
    /// application. It then triggers subgraph verification for relevant
    /// pairs. In case of a load error, it handles the error by calling
    /// `on_state_load_error`.

    fn on_pool_resolve(&mut self, state: LazyResult) {
        let LazyResult { block, state, load_result } = state;

        if let Some(state) = state {
            let addr = state.address();

            self.graph_manager.new_state(addr, state);

            // pool was initialized this block. lets set the override to avoid invalid state
            if !load_result.is_ok() {
                self.buffer.overrides.entry(block).or_default().insert(addr);
            }
        } else if let LoadResult::Err {
            block,
            pool_address,
            pool_pair,
            protocol,
            deps,
            full_pair,
        } = load_result
        {
            self.new_graph_pairs
                .insert(pool_address, (protocol, pool_pair));

            let failed_queries = deps
                .into_iter()
                .map(|(pair, goes_through)| {
                    self.graph_manager.pool_dep_failure(pair, goes_through);
                    RequeryPairs {
                        full_pair,
                        block,
                        goes_through,
                        pair,
                        frayed_ends: Default::default(),
                        ignore_state: Default::default(),
                    }
                })
                .collect_vec();

            self.requery_bad_state_par(failed_queries)
        }
    }

    /// Attempts to verify subgraphs for a given set of pairs and handles the
    /// verification results.
    ///
    /// # Behavior
    /// The function triggers subgraph verification for each provided pair and
    /// block number combination. On successful verification, it prunes bad
    /// edges from the subgraph and updates the graph manager with the verified
    /// subgraph. If verification fails, it prunes bad edges and prepares
    /// the failed pair for requery. After processing the verification
    /// results, it requeues any pairs that need to be reverified due to failed
    /// verification.
    fn try_verify_subgraph(&mut self, pairs: Vec<(u64, Option<u64>, Pair, Vec<Pair>)>) {
        let requery = self
            .graph_manager
            .verify_subgraph(pairs, self.quote_asset)
            .into_iter()
            .filter_map(|result| match result {
                VerificationResults::Passed(passed) => {
                    passed.prune_state.into_iter().for_each(|(_, bad_edges)| {
                        for bad_edge in bad_edges {
                            if let Some((addr, protocol, pair)) = self
                                .graph_manager
                                .remove_pair_graph_address(bad_edge.pair, bad_edge.pool_address)
                            {
                                self.new_graph_pairs.insert(addr, (protocol, pair));
                            }
                        }
                    });
                    self.graph_manager.add_verified_subgraph(
                        passed.pair,
                        passed.subgraph,
                        passed.block,
                    );

                    None
                }
                VerificationResults::Failed(failed) => {
                    failed.prune_state.into_iter().for_each(|(_, bad_edges)| {
                        for bad_edge in bad_edges {
                            if let Some((addr, protocol, pair)) = self
                                .graph_manager
                                .remove_pair_graph_address(bad_edge.pair, bad_edge.pool_address)
                            {
                                self.new_graph_pairs.insert(addr, (protocol, pair));
                            }
                        }
                    });

                    Some(RequeryPairs {
                        pair:         failed.pair,
                        goes_through: failed.goes_through,
                        block:        failed.block,
                        frayed_ends:  failed.frayed_ends,
                        full_pair:    failed.full_pair,
                        ignore_state: failed.ignore_state,
                    })
                }
                VerificationResults::Abort(pair, goes_through, block) => {
                    self.failed_pairs
                        .entry(block)
                        .or_default()
                        .push((pair, goes_through));

                    None
                }
            })
            .collect_vec();

        self.requery_bad_state_par(requery);
    }

    /// Requeries the state of subgraphs for given pairs that encountered issues
    /// during verification.
    ///
    /// # Behavior
    /// This function is invoked when subgraph verification fails for certain
    /// pairs. It reattempts to construct valid subgraphs by: 1. Requerying
    /// the state for each pair and block number, considering any ignored pairs
    /// during the graph construction. 2. Adding newly constructed subgraphs
    /// if they are non-empty, or recursively removing problematic pairs and
    /// requerying if necessary. 3. In cases where no valid paths are found
    /// after requery, it escalates the verification by analyzing alternative
    /// paths or pairs.
    fn requery_bad_state_par(&mut self, pairs: Vec<RequeryPairs>) {
        if pairs.is_empty() {
            return
        }
        tracing::debug!("requerying bad state");

        let new_state = execute_on!(target = pricing, par_state_query(&self.graph_manager, pairs));

        if new_state.is_empty() {
            error!("requery bad state returned nothing");
        }

        let mut recusing = Vec::new();

        new_state.into_iter().for_each(
            |StateQueryRes { pair, block, edges, extends_pair, goes_through, full_pair }| {
                let edges = edges.into_iter().flatten().unique().collect_vec();
                // add regularly
                if edges.is_empty() {
                    self.rundown(pair, full_pair, goes_through, block);
                    return
                }

                let Some((id, need_state, force_rundown)) = self.add_subgraph(
                    pair,
                    full_pair,
                    goes_through,
                    extends_pair,
                    block,
                    edges,
                    true,
                ) else {
                    return;
                };

                if force_rundown {
                    self.rundown(pair, full_pair, goes_through, block);
                } else if !need_state {
                    recusing.push((block, id, pair, vec![goes_through]))
                }
            },
        );

        if !recusing.is_empty() {
            execute_on!(target = pricing, self.try_verify_subgraph(recusing));
        }
        tracing::debug!("finished requerying bad state");
    }

    /// rundown occurs when we have hit a attempt limit for trying to find high
    /// liquidity nodes for a pair subgraph. when this happens, we take all
    /// of the low liquidity nodes and generate all unique paths through each
    /// and then add it to the subgraph. And then allow for these low liquidity
    /// nodes as they are the only nodes for the given pair.
    fn rundown(&mut self, pair: Pair, complete_pair: Pair, goes_through: Pair, block: u64) {
        let Some(ignores) = self
            .graph_manager
            .verify_subgraph_on_new_path_failure(pair, &goes_through)
        else {
            return;
        };

        if ignores.is_empty() {
            tracing::debug!(
                ?pair,
                ?block,
                "rundown for subgraph has no edges we are supposed to ignore"
            );
        }

        // take all combinations of our ignore nodes
        let queries = if ignores.len() > 1 {
            ignores
                .iter()
                .copied()
                .combinations(ignores.len() - 1)
                .map(|ignores| RequeryPairs {
                    pair,
                    goes_through,
                    full_pair: complete_pair,
                    block,
                    ignore_state: ignores.into_iter().collect::<FastHashSet<_>>(),
                    frayed_ends: vec![],
                })
                .collect_vec()
        } else {
            vec![RequeryPairs {
                goes_through,
                pair,
                block,
                full_pair: complete_pair,
                ignore_state: FastHashSet::default(),
                frayed_ends: vec![],
            }]
        };

        tracing::debug!(?pair, ?block, subgraph_variations = queries.len(), "starting rundown");

        let (edges, extend) = execute_on!(target = pricing, {
            let (edges, mut extend): (Vec<_>, Vec<_>) =
                par_state_query(&self.graph_manager, queries)
                    .into_iter()
                    .map(|e| (e.edges, e.extends_pair))
                    .unzip();

            let edges = edges.into_iter().flatten().flatten().unique().collect_vec();

            // if we dont have any edges, lets run with no ignores.
            if edges.is_empty() {
                let query = vec![RequeryPairs {
                    goes_through,
                    pair,
                    block,
                    full_pair: complete_pair,
                    ignore_state: FastHashSet::default(),
                    frayed_ends: vec![],
                }];

                let (edges, mut extend): (Vec<_>, Vec<_>) =
                    par_state_query(&self.graph_manager, query)
                        .into_iter()
                        .map(|e| (e.edges, e.extends_pair))
                        .unzip();

                (
                    edges.into_iter().flatten().flatten().unique().collect_vec(),
                    extend.pop().flatten(),
                )
            } else {
                (edges, extend.pop().flatten())
            }
        });

        if edges.is_empty() {
            error!(?pair, ?block, "failed to find connection for graph");
            return
        } else {
            let Some((id, need_state, _)) =
                self.add_subgraph(pair, complete_pair, goes_through, extend, block, edges, true)
            else {
                return;
            };

            if !need_state {
                execute_on!(
                    target = pricing,
                    self.try_verify_subgraph(vec![(block, id, pair, vec![goes_through])])
                );
            }
        }
        tracing::debug!(?pair, ?block, "finished rundown");
    }

    /// Adds a subgraph for verification based on the given pair, block, and
    /// edges.
    ///
    /// # Behavior
    /// This function is responsible for initializing the process of verifying a
    /// new subgraph. It involves: 1. Adding the subgraph to the
    /// verification queue with the necessary edges and state. 2. Initiating
    /// lazy loading for the exchange pools involved in the subgraph if they are
    /// not already being loaded. 3. Adding the pool as a dependent to an
    /// ongoing load operation if it's already in progress.
    ///
    /// The function returns a boolean indicating whether any lazy loading was
    /// triggered during its execution. This function ensures that all necessary
    /// pool states are loaded and ready for accurate subgraph verification.
    fn add_subgraph(
        &mut self,
        pair: Pair,
        complete_pair: Pair,
        goes_through: Pair,
        extends_to: Option<Pair>,
        block: u64,
        edges: Vec<SubGraphEdge>,
        frayed_ext: bool,
    ) -> Option<(Option<u64>, bool, bool)> {
        let (needed_state, id, force_rundown) = if frayed_ext {
            let (need, id, force_rundown) =
                self.graph_manager
                    .add_frayed_end_extension(pair, &goes_through, block, edges)?;
            (need, Some(id), force_rundown)
        } else {
            (
                self.graph_manager.add_subgraph_for_verification(
                    pair,
                    complete_pair,
                    goes_through,
                    extends_to,
                    block,
                    edges,
                ),
                None,
                false,
            )
        };

        let mut triggered = false;
        // because we run these state fetches in parallel, we come across the issue
        // where in block N we have a path, it however doesn't get verified so we go to
        // query more state. however the new path it takes goes through a pool that is
        // being inited with state from block N + I, when we go to calculate the price
        // the state will be off thus giving us a incorrect price
        for pool_info in needed_state {
            let is_lazy_loading =
                if let Some(blocks) = self.lazy_loader.is_loading_block(&pool_info.pool_addr) {
                    blocks.contains(&block)
                } else {
                    false
                };

            if !is_lazy_loading {
                self.lazy_loader.lazy_load_exchange(
                    pair,
                    Pair(pool_info.token_0, pool_info.token_1),
                    goes_through,
                    complete_pair,
                    id,
                    pool_info.pool_addr,
                    block,
                    pool_info.dex_type,
                );
                triggered = true;
            } else {
                self.lazy_loader.add_protocol_parent(
                    block,
                    id,
                    pool_info.pool_addr,
                    pair,
                    goes_through,
                );
                triggered = true;
            }
        }

        Some((id, triggered, force_rundown))
    }

    fn can_progress(&self) -> bool {
        self.lazy_loader.can_progress(&self.completed_block)
            && self.completed_block < self.current_block
    }

    /// The price can pre-process up to 10 blocks in the future
    fn process_future_blocks(&self) -> bool {
        self.completed_block + 10 > self.current_block
    }

    /// Attempts to resolve the block & start processing the next block.
    fn try_resolve_block(&mut self) -> Option<(u64, DexQuotes)> {
        // if there are still requests for the given block or the current block isn't
        // complete yet, then we wait
        if !self.can_progress() {
            return None
        }

        self.graph_manager.finalize_block(self.completed_block);

        // if all block requests are complete, lets apply all the state transitions we
        // had for the given block which will allow us to generate all pricing
        let (buffer, overrides) = (
            self.buffer
                .updates
                .remove(&self.completed_block)
                .unwrap_or_default(),
            self.buffer
                .overrides
                .remove(&self.completed_block)
                .unwrap_or_default(),
        );

        for (address, update) in buffer {
            if overrides.contains(&address) {
                // we will just init the pool but nothing else since the state of the pool is
                // end of block
                self.init_new_pool_override(address, update)
            } else {
                // make sure to apply state updates
                self.update_known_state(address, update);
            }
        }

        let block = self.completed_block;

        let mut res = self
            .dex_quotes
            .remove(&self.completed_block)
            .unwrap_or(DexQuotes(vec![]));

        self.handle_drastic_price_changes(&mut res);

        self.completed_block += 1;

        // add new nodes to pair graph
        Some((block, res))
    }

    /// For the given DexQuotes, checks to see if the start price vs the end
    /// price contains a drastic change. This is done to avoid incorrect
    /// prices. prices can have drastic changes within the block (think
    /// sandwich for example). However we know that any incorrect price
    /// should be corrected before the end of the block. We use this knowledge
    /// to see if the price had a massive valid change or is just being
    /// manipulated for mev.
    fn handle_drastic_price_changes(&mut self, prices: &mut DexQuotes) {
        let mut first = FastHashMap::default();
        let mut last = FastHashMap::default();

        prices
            .0
            .iter()
            .filter_map(|p| p.as_ref())
            .for_each(|tx_prices| {
                for (k, p) in tx_prices {
                    let gt = p.goes_through;
                    if let Entry::Vacant(v) = first.entry((*k, gt)) {
                        v.insert(p.clone().get_price(PriceAt::Before));
                        last.insert((*k, gt), p.clone().get_price(PriceAt::After));
                    } else {
                        last.insert((*k, gt), p.clone().get_price(PriceAt::After));
                    }
                }
            });

        // all pairs over max price movement
        let removals = first
            .into_iter()
            .filter_map(|(key, price)| Some((key, price, last.remove(&key)?)))
            .filter_map(|(key, first_price, last_price)| {
                let block_movement = (last_price - &first_price).abs() / first_price;
                if block_movement > MAX_BLOCK_MOVEMENT {
                    Some(key)
                } else {
                    None
                }
            })
            .collect::<FastHashSet<_>>();

        prices
            .0
            .iter_mut()
            .filter_map(|p| p.as_mut())
            .for_each(|map| map.retain(|k, v| !removals.contains(&(*k, v.goes_through))));

        removals.into_iter().for_each(|pair| {
            tracing::debug!(pair=?pair.0, goes_through=?pair.1, "drastic price change detected. removing pair");
            self.graph_manager.remove_subgraph(pair.0, pair.1);
        })
    }

    fn on_close(&mut self) -> Option<(u64, DexQuotes)> {
        if self.completed_block > self.current_block {
            return None
        }

        self.graph_manager.finalize_block(self.completed_block);

        // if all block requests are complete, lets apply all the state transitions we
        // had for the given block which will allow us to generate all pricing
        let (buffer, overrides) = (
            self.buffer
                .updates
                .remove(&self.completed_block)
                .unwrap_or_default(),
            self.buffer
                .overrides
                .remove(&self.completed_block)
                .unwrap_or_default(),
        );

        for (address, update) in buffer {
            if overrides.contains(&address) {
                self.init_new_pool_override(address, update)
            } else {
                self.update_known_state(address, update);
            }
        }

        let block = self.completed_block;

        let mut res = self
            .dex_quotes
            .remove(&self.completed_block)
            .unwrap_or(DexQuotes(vec![]));

        self.handle_drastic_price_changes(&mut res);

        self.completed_block += 1;

        Some((block, res))
    }

    fn poll_state_processing(
        &mut self,
        cx: &mut Context<'_>,
    ) -> Option<Poll<Option<(u64, DexQuotes)>>> {
        // because results tend to stack up, we always want to progress them first
        while let Poll::Ready(Some(state)) = self.lazy_loader.poll_next_unpin(cx) {
            self.on_pool_resolve(state)
        }

        let pairs = self.lazy_loader.pairs_to_verify();

        execute_on!(target = pricing, self.try_verify_subgraph(pairs));

        // check if we can progress to the next block.
        self.try_resolve_block()
            .map(|prices| Poll::Ready(Some(prices)))
    }
}

impl<T: TracingProvider, DB: LibmdbxReader + DBWriter + Unpin> Stream
    for BrontesBatchPricer<T, DB>
{
    type Item = (u64, DexQuotes);

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let mut work = 8;
        loop {
            work -= 1;
            if work == 0 {
                cx.waker().wake_by_ref();
                return Poll::Pending
            }

            if let Some(new_prices) = self.poll_state_processing(cx) {
                return new_prices
            }

            if !self.process_future_blocks() {
                continue
            }

            let mut block_updates = Vec::new();
            loop {
                match self.update_rx.poll_recv(cx).map(|inner| {
                    inner.and_then(|action| match action {
                        DexPriceMsg::DisablePricingFor(block) => {
                            self.skip_pricing.push_back(block);
                            tracing::debug!(?block, "skipping pricing");
                            Some(PollResult::Skip)
                        }
                        DexPriceMsg::Update(update) => Some(PollResult::State(update)),
                        DexPriceMsg::DiscoveredPool(NormalizedPoolConfigUpdate {
                            protocol,
                            tokens,
                            pool_address,
                            ..
                        }) => {
                            if protocol.has_state_updater() {
                                self.new_graph_pairs
                                    .insert(pool_address, (protocol, Pair(tokens[0], tokens[1])));
                            };
                            Some(PollResult::DiscoveredPool)
                        }
                        DexPriceMsg::Closed => None,
                    })
                }) {
                    Poll::Ready(Some(u)) => {
                        if let PollResult::State(update) = u {
                            if let Some(overlap) = self.overlap_update.take() {
                                block_updates.push(overlap);
                            }
                            if update.block == self.current_block {
                                block_updates.push(update);
                            } else {
                                self.overlap_update = Some(update);
                                break
                            }
                        }
                    }
                    Poll::Ready(None) | Poll::Pending => {
                        if self.lazy_loader.is_empty()
                            && self.lazy_loader.can_progress(&self.completed_block)
                            && block_updates.is_empty()
                            && self.finished.load(SeqCst)
                        {
                            return Poll::Ready(self.on_close())
                        }
                        break
                    }
                }

                // we poll here to continuously progress state fetches as they are slow
                if let Poll::Ready(Some(state)) = self.lazy_loader.poll_next_unpin(cx) {
                    self.on_pool_resolve(state);
                }
            }

            #[allow(clippy::blocks_in_conditions)]
            if block_updates
                .first()
                .map(|u| u.block)
                .and_then(|block_update_num| {
                    // remove all blocks before the current block
                    self.skip_pricing.retain(|block| block >= &block_update_num);
                    let front = self.skip_pricing.front()?;
                    Some(&block_update_num == front)
                })
                .unwrap_or(false)
            {
                self.on_pool_update_no_pricing(block_updates);
            } else {
                execute_on!(target = pricing, self.on_pool_updates(block_updates));
            }
        }
    }
}

enum PollResult {
    State(PoolUpdate),
    DiscoveredPool,
    Skip,
}

/// a ordered buffer for holding state transitions for a block while the lazy
/// loading of pools is being applied
pub struct StateBuffer {
    /// updates for a given block in order that they occur
    pub updates:   FastHashMap<u64, VecDeque<(Address, PoolUpdate)>>,
    /// when we have a override for a given address at a block. it means that
    /// we don't want to apply any pool updates for the block. This is useful
    /// for when a pool is  at a block and we can only query the end
    /// of block state. we can override all pool updates for the init block
    /// to ensure our pool state is in sync
    pub overrides: FastHashMap<u64, FastHashSet<Address>>,
}

impl Default for StateBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl StateBuffer {
    pub fn new() -> Self {
        Self { updates: FastHashMap::default(), overrides: FastHashMap::default() }
    }
}

#[cfg(feature = "tests")]
impl<T: TracingProvider, DB: DBWriter + LibmdbxReader> BrontesBatchPricer<T, DB> {
    pub fn get_lazy_loader(&mut self) -> &mut LazyExchangeLoader<T> {
        &mut self.lazy_loader
    }

    pub fn get_buffer(&mut self) -> &mut StateBuffer {
        &mut self.buffer
    }
}

#[cfg(all(test, feature = "local-reth"))]
pub mod test {
    use std::sync::Arc;

    use brontes_classifier::test_utils::ClassifierTestUtils;
    use brontes_types::{
        constants::USDC_ADDRESS,
        db::dex::{DexPrices, DexQuotes},
        pair::Pair,
        ToFloatNearest,
    };
    use futures::future::join_all;
    use itertools::Itertools;
    use malachite::Rational;

    use crate::FastHashMap;

    // takes to long if using http
    #[brontes_macros::test(threads = 11)]
    async fn test_pricing_variance() {
        let utils = Arc::new(ClassifierTestUtils::new().await);
        let bad_block = 18500018;
        let mut dex_quotes: Vec<DexQuotes> = join_all((0..4).map(|_| {
            let c = utils.clone();
            tokio::spawn(async move {
                c.build_block_tree_with_pricing(bad_block, USDC_ADDRESS, vec![])
                    .await
                    .unwrap()
                    .1
                    .unwrap()
            })
        }))
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();

        // generate a bitmap of all locations that are valid
        let last = dex_quotes.pop().unwrap();
        let mut expected = vec![0u8; last.0.len()];

        last.0.iter().enumerate().for_each(|(i, p_entry)| {
            if p_entry.is_some() {
                expected[i / 8] |= 1 << (i % 8);
            }
        });

        // verify all align
        dex_quotes.iter().for_each(|quotes| {
            quotes.0.iter().enumerate().for_each(|(i, p_entry)| {
                if p_entry.is_some() {
                    assert!(
                        expected[i / 8] & 1 << (i % 8) != 0,
                        "have a entry where another generation doesn't: tx {i}"
                    )
                } else {
                    assert!(
                        expected[i / 8] & 1 << (i % 8) == 0,
                        "missing a entry where another generation has one: tx {i}"
                    )
                }
            });
        });

        let pair_to_batch_to_dex_price = dex_quotes
            .iter()
            .chain([last].iter())
            .map(|i| &i.0)
            .flat_map(|quotes: &Vec<Option<FastHashMap<Pair, DexPrices>>>| {
                quotes
                    .iter()
                    .filter_map(|a| a.as_ref())
                    .flat_map(|a| a.iter().map(|(p, b)| (*p, b.clone())))
                    .into_group_map()
                    .into_iter()
            })
            .into_group_map();

        // check to make sure all prices are within 1% of each other over the batches
        pair_to_batch_to_dex_price
            .into_iter()
            .for_each(|(pair, prices)| {
                // with prices, its
                // [batch [ position in batch ]]
                // calcuate the average for each index of prices
                let inner_len = prices[0].len();
                for i in 0..inner_len {
                    let mut pre_prices = vec![];
                    let mut post_prices = vec![];
                    for price in &prices {
                        pre_prices.push(price[i].pre_state.clone());
                        post_prices.push(price[i].post_state.clone());
                    }
                    // // check min max diff is below th
                    let pre_min = pre_prices.iter().min().unwrap();
                    let pre_max = pre_prices.iter().max().unwrap();

                    let diff = (pre_max - pre_min) / pre_max * Rational::from(100);

                    if diff > Rational::const_from_unsigneds(1, 10000) {
                        panic!("{:?} pre state had a diff of: {}%", pair, diff.to_float());
                    }

                    let post_min = pre_prices.iter().min().unwrap();
                    let post_max = pre_prices.iter().max().unwrap();

                    let diff = (post_max - post_min) / post_max * Rational::from(100);

                    if diff > Rational::const_from_unsigneds(1, 10000) {
                        panic!("{:?} pre state had a diff of: {}%", pair, diff.to_float());
                    }
                }
            })
    }
}
