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
        self.all_pair_graph
            .add_node(pair.ordered(), pool_addr, dex, block);
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
    ) -> Vec<SubGraphEdge> {
        let ordered_pair = pair.ordered();

        if let Ok((_, edges)) = self.db.try_load_pair_before(block, ordered_pair.ordered()) {
            info!("db load");
            return edges
        }

        let paths = self
            .all_pair_graph
            .get_paths_ignoring(pair, &ignore, block)
            .into_iter()
            .flatten()
            .flatten()
            .collect_vec();

        paths
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
    pub fn create_subgraph_mut(&mut self, block: u64, pair: Pair) -> Vec<PoolPairInfoDirection> {
        let ordered_pair = pair.ordered();

        if let Ok((pair, edges)) = self.db.try_load_pair_before(block, ordered_pair) {
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
            .get_paths(pair, block)
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

    pub fn bad_pool_state(
        &mut self,
        subgraph_pair: Pair,
        pool_pair: Pair,
        pool_address: Address,
    ) -> (bool, Option<(Address, Protocol, Pair)>) {
        let requery_subgraph = self.sub_graph_registry.bad_pool_state(
            subgraph_pair.ordered(),
            pool_pair.ordered(),
            pool_address,
        );

        (
            requery_subgraph,
            self.all_pair_graph
                .remove_empty_address(pool_pair, pool_address),
        )
    }

    pub fn add_verified_subgraph(&mut self, pair: Pair, subgraph: PairSubGraph) {
        self.sub_graph_registry
            .add_verified_subgraph(pair.ordered(), subgraph)
    }

    pub fn remove_pair_graph_address(
        &mut self,
        pool_pair: Pair,
        pool_address: Address,
    ) -> Option<(Address, Protocol, Pair)> {
        self.all_pair_graph
            .remove_empty_address(pool_pair, pool_address)
    }

    pub fn verify_subgraph_on_new_path_failure(&mut self, pair: Pair) -> Option<Vec<Pair>> {
        self.subgraph_verifier
            .verify_subgraph_on_new_path_failure(pair.ordered())
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
        self.sub_graph_registry.has_subpool(&pair.ordered())
            || self.subgraph_verifier.is_verifying(&pair.ordered())
    }

    pub fn verify_subgraph(
        &mut self,
        pairs: Vec<(u64, Pair)>,
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
}
