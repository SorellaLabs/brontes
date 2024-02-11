mod all_pair_graph;
mod dijkstras;
mod registry;
mod state_tracker;
mod subgraph;
mod yens;
use std::collections::{HashMap, HashSet};
mod subgraph_verifier;
pub use all_pair_graph::AllPairGraph;
use alloy_primitives::Address;
use brontes_types::{
    db::traits::{LibmdbxReader, LibmdbxWriter},
    pair::Pair,
    price_graph_types::{PoolPairInfoDirection, SubGraphEdge},
};
use itertools::Itertools;
use malachite::Rational;
pub use subgraph_verifier::VerificationResults;
use tracing::info;

use self::{
    registry::SubGraphRegistry, state_tracker::StateTracker, subgraph::PairSubGraph,
    subgraph_verifier::*,
};
use super::PoolUpdate;
use crate::{types::PoolState, Protocol};

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
pub struct GraphManager<DB: LibmdbxReader + LibmdbxWriter> {
    all_pair_graph:     AllPairGraph,
    /// registry of all finalized subgraphs
    sub_graph_registry: SubGraphRegistry,
    /// deals with the verification process of our subgraphs
    subgraph_verifier:  SubgraphVerifier,
    /// tracks all state needed for our subgraphs
    graph_state:        StateTracker,
    /// allows us to save a load subgraphs.
    db:                 &'static DB,
}

impl<DB: LibmdbxWriter + LibmdbxReader> GraphManager<DB> {
    pub fn init_from_db_state(
        all_pool_data: HashMap<(Address, Protocol), Pair>,
        sub_graph_registry: HashMap<Pair, Vec<SubGraphEdge>>,
        db: &'static DB,
    ) -> Self {
        let graph = AllPairGraph::init_from_hashmap(all_pool_data);
        let registry = SubGraphRegistry::new(sub_graph_registry);
        let subgraph_verifier = SubgraphVerifier::new();

        Self {
            graph_state: StateTracker::new(),
            all_pair_graph: graph,
            sub_graph_registry: registry,
            db,
            subgraph_verifier,
        }
    }

    pub fn add_pool(&mut self, pair: Pair, pool_addr: Address, dex: Protocol, block: u64) {
        self.all_pair_graph.add_node(pair, pool_addr, dex, block);
    }

    pub fn all_verifying_pairs(&self) -> Vec<Pair> {
        self.subgraph_verifier.all_pairs()
    }

    /// creates a subgraph returning the edges and the state to load.
    /// this is done so that this isn't mut and be ran in parallel
    pub fn create_subgraph(
        &self,
        block: u64,
        pair: Pair,
        ignore: HashSet<Pair>,
        connectivity_wight: usize,
        connections: usize,
    ) -> Vec<SubGraphEdge> {
        let ordered_pair = pair.ordered();

        if let Ok((_, edges)) = self.db.try_load_pair_before(block, ordered_pair.ordered()) {
            info!("db load");
            return edges
        }

        self.all_pair_graph
            .get_paths_ignoring(pair, &ignore, block, connectivity_wight, connections)
            .into_iter()
            .flatten()
            .flatten()
            .collect_vec()
    }

    pub fn add_subgraph_for_verification(
        &mut self,
        pair: Pair,
        block: u64,
        edges: Vec<SubGraphEdge>,
    ) -> Vec<PoolPairInfoDirection> {
        self.subgraph_verifier
            .create_new_subgraph(pair, block, edges, &self.graph_state)
    }

    /// creates a subpool for the pair returning all pools that need to be
    /// loaded
    pub fn create_subgraph_mut(
        &mut self,
        block: u64,
        pair: Pair,
        connectivity_wight: usize,
        connections: usize,
    ) -> Vec<PoolPairInfoDirection> {
        let ordered_pair = pair.ordered();

        if let Ok((pair, edges)) = self.db.try_load_pair_before(block, ordered_pair) {
            info!("db load");
            return self
                .subgraph_verifier
                .create_new_subgraph(pair, block, edges, &self.graph_state)
        }

        let paths = self
            .all_pair_graph
            // We want to use the unordered pair as we always
            // want to run the search from the unkown token to the quote.
            // We want this beacuse our algorithm favors heavily connected
            // nodes which most times our base token is not. This small
            // change speeds up yens algo by a good amount.
            .get_paths(pair, block, connectivity_wight, connections)
            .into_iter()
            .flatten()
            .flatten()
            .collect_vec();

        // search failed
        if paths.is_empty() {
            info!(?pair, "empty search path");
            return vec![]
        }

        self.subgraph_verifier
            .create_new_subgraph(pair, block, paths, &self.graph_state)
    }

    pub fn add_verified_subgraph(&mut self, pair: Pair, subgraph: PairSubGraph, block: u64) {
        if let Err(e) =
            self.db
                .save_pair_at(block, pair, subgraph.get_all_pools().cloned().collect())
        {
            tracing::error!(error = e, "failed to save new subgraph pair");
        }
        self.sub_graph_registry.add_verified_subgraph(
            pair,
            subgraph,
            &self.graph_state.all_state(block),
        )
    }

    pub fn remove_pair_graph_address(
        &mut self,
        pool_pair: Pair,
        pool_address: Address,
    ) -> Option<(Address, Protocol, Pair)> {
        self.all_pair_graph
            .remove_empty_address(pool_pair, pool_address)
    }

    /// Returns all pairs that have been ignored from lowest to highest
    /// liquidity
    pub fn verify_subgraph_on_new_path_failure(&mut self, pair: Pair) -> Option<Vec<Pair>> {
        self.subgraph_verifier
            .verify_subgraph_on_new_path_failure(pair)
    }

    pub fn get_price(&self, pair: Pair) -> Option<Rational> {
        self.sub_graph_registry
            .get_price(pair, self.graph_state.finalized_state())
    }

    pub fn new_state(&mut self, address: Address, state: PoolState) {
        self.graph_state.new_state_for_verification(address, state);
    }

    pub fn update_state(&mut self, address: Address, update: PoolUpdate) {
        self.graph_state.update_pool_state(address, update);
    }

    pub fn has_subgraph(&self, pair: Pair) -> bool {
        self.sub_graph_registry.has_subpool(&pair) || self.subgraph_verifier.is_verifying(&pair)
    }

    pub fn remove_state(&mut self, address: &Address) {
        self.graph_state.remove_state(address)
    }

    pub fn add_frayed_end_extension(
        &mut self,
        pair: Pair,
        block: u64,
        frayed_end_extensions: Vec<SubGraphEdge>,
    ) -> Option<(Vec<PoolPairInfoDirection>, u64, bool)> {
        self.subgraph_verifier.add_frayed_end_extension(
            pair,
            block,
            &self.graph_state,
            frayed_end_extensions,
        )
    }

    pub fn verify_subgraph(
        &mut self,
        pairs: Vec<(u64, Option<u64>, Pair)>,
        quote: Address,
    ) -> Vec<VerificationResults> {
        self.subgraph_verifier.verify_subgraph(
            pairs,
            quote,
            &self.all_pair_graph,
            &mut self.graph_state,
        )
    }

    pub fn finalize_block(&mut self, block: u64) {
        self.graph_state.finalize_block(block);
    }

    /// removes all subgraphs that have a pool that's current liquidity
    /// is less than its liquidity when it was verified.
    /// nothing is done as we won't bother re-verifying until pricing for the
    /// graph is needed again
    pub fn audit_subgraphs(&mut self) {
        self.sub_graph_registry
            .audit_subgraphs(self.graph_state.finalized_state())
    }
}
