mod all_pair_graph;
mod dijkstras;
mod registry;
mod state_tracker;
mod subgraph;
mod yens;
use std::{sync::Arc, time::Duration};

use brontes_metrics::pricing::DexPricingMetrics;
use brontes_types::{FastHashMap, FastHashSet};
use parking_lot::RwLock;
mod subgraph_verifier;
use std::pin::Pin;

pub use all_pair_graph::AllPairGraph;
use alloy_primitives::Address;
use brontes_types::{
    pair::Pair,
    price_graph_types::{PoolPairInfoDirection, SubGraphEdge},
};
use futures::Future;
use itertools::Itertools;
use malachite::{num::basic::traits::One, Rational};
use tracing::error_span;

pub use self::{
    registry::SubGraphRegistry,
    state_tracker::{StateTracker, StateWithDependencies},
    subgraph::{PairSubGraph, VerificationOutcome},
    subgraph_verifier::*,
};
use super::{PendingHeavyCalcs, PoolUpdate};
use crate::{
    types::{PairWithFirstPoolHop, PoolState},
    Protocol,
};

/// [`GraphManager`] Is the manager for everything graph related. It is
/// responsible for creating, updating, and maintaining the main token graph as
/// well as its derived subgraphs.
///
/// ## Subgraph Management
/// - **Subgraph Registry**: Maintains a collection of subgraphs
///   (`SubGraphRegistry`) for efficient price calculation.
/// - **Subgraph Creation and Verification**: Generates and verifies new
///   subgraphs that allow for pricing of any token pair.
/// - **Handling Bad Pool States**: Addresses problematic pools within subgraphs
///   to ensure accurate pricing data.
///
/// ## Price Calculation and State Tracking
/// - **Price Retrieval**: Retrieves prices for specific token pairs based on
///   their subgraphs.
/// - **State Management**: Tracks and monitors the changes in pools over time
///   through `graph_state` (`StateTracker`).
///
/// ## Operational Flow
/// - **Initialization**: Initializes the `GraphManager` with existing pool data
///   and subgraphs from the database.
/// - **Adding Pools and Verifying Subgraphs**: Adds new pools and verifies the
///   integrity of associated subgraphs.
/// - **Finalizing Blocks**: Concludes the processing of a block, finalizing the
///   state for the generated subgraphs.
#[derive(Clone)]
pub struct GraphManager {
    all_pair_graph:               Arc<RwLock<AllPairGraph>>,
    /// registry of all finalized subgraphs
    sub_graph_registry:           Arc<RwLock<SubGraphRegistry>>,
    /// deals with the verification process of our subgraphs
    pub(crate) subgraph_verifier: Arc<RwLock<SubgraphVerifier>>,
    /// tracks all state needed for our subgraphs
    graph_state:                  Arc<RwLock<StateTracker>>,
}

impl GraphManager {
    pub fn init_from_db_state(
        all_pool_data: FastHashMap<(Address, Protocol), Pair>,
        metrics: Option<DexPricingMetrics>,
    ) -> Self {
        let graph = Arc::new(RwLock::new(AllPairGraph::init_from_hash_map(all_pool_data)));
        let registry = Arc::new(RwLock::new(SubGraphRegistry::new(metrics.clone())));
        let subgraph_verifier = Arc::new(RwLock::new(SubgraphVerifier::new()));

        Self {
            graph_state: Arc::new(RwLock::new(StateTracker::new(metrics))),
            all_pair_graph: graph,
            sub_graph_registry: registry,
            subgraph_verifier,
        }
    }

    /// used for testing and benching
    pub fn snapshot_state(&self) -> (SubGraphRegistry, SubgraphVerifier, StateTracker) {
        (
            (*self.sub_graph_registry.clone()).read().clone(),
            (*self.subgraph_verifier.clone()).read().clone(),
            (*self.graph_state.clone()).read().clone(),
        )
    }

    /// used for testing and benching
    pub fn set_state(
        &mut self,
        sub_graph_registry: SubGraphRegistry,
        verifier: SubgraphVerifier,
        state: StateTracker,
    ) {
        self.sub_graph_registry = Arc::new(RwLock::new(sub_graph_registry));
        self.subgraph_verifier = Arc::new(RwLock::new(verifier));
        self.graph_state = Arc::new(RwLock::new(state));
    }

    pub fn add_pool(&self, pair: Pair, pool_addr: Address, dex: Protocol, block: u64) {
        self.all_pair_graph
            .write()
            .add_node(pair, pool_addr, dex, block);
    }

    pub fn pool_dep_failure(
        &self,
        pair: &PairWithFirstPoolHop,
        pool_addr: Address,
        pool_pair: Pair,
    ) -> bool {
        self.subgraph_verifier
            .write()
            .pool_dep_failure(pair, pool_addr, pool_pair)
    }

    pub fn has_extension(&self, pair: &Pair, quote: Address) -> Option<Pair> {
        self.sub_graph_registry.read().has_extension(pair, quote)
    }

    pub fn mark_future_use(&self, pair: Pair, goes_through: Pair, block: u64) {
        self.sub_graph_registry
            .read()
            .mark_future_use(pair, goes_through, block);
    }

    /// creates a subgraph returning the edges and the state to load.
    /// this is done so that this isn't mut and be ran in parallel
    pub fn create_subgraph(
        &self,
        block: u64,
        first_hop: Option<Pair>,
        pair: Pair,
        ignore: FastHashSet<Pair>,
        connectivity_wight: usize,
        connections: Option<usize>,
        timeout: Duration,
        is_extension: bool,
        trying_extensions_quote: Option<Address>,
    ) -> (Vec<SubGraphEdge>, Option<Pair>) {
        let possible_exts = trying_extensions_quote
            .map(|quote| {
                self.sub_graph_registry
                    .read()
                    .all_pairs_with_quote_for_extends(quote)
            })
            .unwrap_or_default();

        let (path, extends) = self.all_pair_graph.read().get_paths_ignoring(
            pair,
            first_hop,
            &ignore,
            block,
            connectivity_wight,
            connections,
            timeout,
            is_extension,
            possible_exts,
        );

        (path.into_iter().flatten().flatten().collect_vec(), extends)
    }

    pub fn add_subgraph_for_verification(
        &self,
        pair: PairWithFirstPoolHop,
        extends_to: Option<Pair>,
        block: u64,
        edges: Vec<SubGraphEdge>,
    ) -> Vec<PoolPairInfoDirection> {
        self.subgraph_verifier.write().create_new_subgraph(
            pair,
            extends_to,
            block,
            edges,
            self.graph_state.clone(),
        )
    }

    /// prunes dead sup_graphs and empty state.
    pub fn prune_dead_subgraphs(&self, block: u64) {
        self.sub_graph_registry
            .write()
            .prune_dead_subgraphs(block)
            .into_iter()
            .for_each(|(pool, amount)| {
                self.graph_state
                    .write()
                    .remove_finalized_state_dep(pool, amount);
            })
    }

    pub fn add_verified_subgraph(&self, subgraph: PairSubGraph, block: u64) {
        self.sub_graph_registry.write().add_verified_subgraph(
            subgraph,
            self.graph_state.read().all_state(block),
            block,
        )
    }

    pub fn remove_pair_graph_address(
        &self,
        pool_pair: Pair,
        pool_address: Address,
    ) -> Option<(Address, Protocol, Pair)> {
        self.all_pair_graph
            .write()
            .remove_empty_address(pool_pair, pool_address)
    }

    pub fn mark_subgraph_for_removal(&self, pair: PairWithFirstPoolHop, block: u64) {
        self.sub_graph_registry
            .write()
            .mark_subgraph_for_removal(pair, block);
    }

    /// Returns all pairs that have been ignored from lowest to highest
    /// liquidity
    pub fn verify_subgraph_on_new_path_failure(
        &self,
        pair: PairWithFirstPoolHop,
    ) -> Option<Vec<Pair>> {
        self.subgraph_verifier
            .write()
            .verify_subgraph_on_new_path_failure(pair)
    }

    pub fn subgraph_extends(&self, pair: PairWithFirstPoolHop) -> Option<Pair> {
        self.subgraph_verifier.read().get_subgraph_extends(pair)
    }

    pub fn get_price(&self, pair: Pair, goes_through: Pair) -> Option<Rational> {
        let span = error_span!("price generation for block");
        span.in_scope(|| {
            self.sub_graph_registry.read().get_price(
                pair,
                goes_through,
                &self.graph_state.read().finalized_state(),
            )
        })
    }

    pub fn new_state(&self, address: Address, state: StateWithDependencies) {
        self.graph_state
            .write()
            .new_state_for_verification(address, state);
    }

    pub fn update_state(&self, address: Address, update: PoolUpdate) {
        self.graph_state.write().update_pool_state(address, update);
    }

    pub fn has_subgraph_goes_through(&self, pair: PairWithFirstPoolHop) -> bool {
        self.sub_graph_registry.read().has_go_through(pair)
            || self.subgraph_verifier.read().has_go_through(pair)
    }

    // returns true if the subgraph should be requeried. will mark it for removal
    // at the current block and it won't be used in pricing in the future
    pub fn prune_low_liq_subgraphs(
        &self,
        pair: PairWithFirstPoolHop,
        quote: Address,
        current_block: u64,
    ) {
        let span = error_span!("verified subgraph pruning");
        span.in_scope(|| {
            let state = self.graph_state.read();
            let state = state.finalized_state();

            // let (start_price, start_addr) = self
            let verifications = self
                .sub_graph_registry
                .read()
                .get_subgraph_extends_iter(pair)
                .into_iter()
                .map(|(epair, jump_pair)| {
                    let (start_price, start_addr) = jump_pair
                        .map(|jump_pair| {
                            (
                                self.sub_graph_registry
                                    .read()
                                    .get_price_all(jump_pair.flip(), &state)
                                    .unwrap_or(Rational::ONE),
                                jump_pair.0,
                            )
                        })
                        .unwrap_or_else(|| (Rational::ONE, quote));
                    (epair, start_addr, start_price)
                })
                .collect::<Vec<_>>();

            self.sub_graph_registry.write().verify_current_subgraphs(
                verifications,
                &state,
                current_block,
            );
        })
    }

    pub fn add_frayed_end_extension(
        &self,
        pair: PairWithFirstPoolHop,
        block: u64,
        frayed_end_extensions: Vec<SubGraphEdge>,
    ) -> Option<(Vec<PoolPairInfoDirection>, u64, bool)> {
        self.subgraph_verifier.write().add_frayed_end_extension(
            pair,
            block,
            self.graph_state.clone(),
            frayed_end_extensions,
        )
    }

    pub fn verify_subgraph(
        &self,
        pairs: Vec<(u64, Option<u64>, PairWithFirstPoolHop)>,
        quote: Address,
    ) -> Pin<Box<dyn Future<Output = PendingHeavyCalcs> + Send>> {
        let span = error_span!("verifying subgraph");
        span.in_scope(|| {
            let pairs = pairs
                .into_iter()
                .map(|(block, id, pair)| {
                    self.subgraph_verifier
                        .read()
                        .get_subgraph_extends(pair)
                        .map(|jump_pair| {
                            (
                                block,
                                id,
                                pair,
                                self.sub_graph_registry
                                    .read()
                                    .get_price_all(
                                        jump_pair.flip(),
                                        &self.graph_state.read().finalized_state(),
                                    )
                                    .unwrap_or(Rational::ONE),
                                jump_pair.0,
                            )
                        })
                        .unwrap_or_else(|| (block, id, pair, Rational::ONE, quote))
                })
                .collect_vec();

            self.subgraph_verifier
                .write()
                .start_verify_subgraph(pairs, self.graph_state.clone())
        })
    }

    pub fn finish_subgraph_verification(
        &self,
        args: Vec<(PairWithFirstPoolHop, u64, VerificationOutcome, Subgraph)>,
    ) -> Vec<VerificationResults> {
        self.subgraph_verifier.write().verify_subgraph_finish(
            args,
            self.all_pair_graph.clone(),
            self.graph_state.clone(),
        )
    }

    pub fn finalize_block(&self, block: u64) {
        self.graph_state.write().finalize_block(block);
        let rem = self.sub_graph_registry.write().finalize_block(block);
        for (pool, amount) in rem {
            self.graph_state
                .write()
                .remove_finalized_state_dep(pool, amount);
        }
    }

    pub fn verification_done_for_block(&self, block: u64) -> bool {
        self.subgraph_verifier.read().is_done_block(block)
    }
}
