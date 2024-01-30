mod all_pair_graph;
mod dijkstras;
mod registry;
mod subgraph;
mod yens;
use std::collections::{HashMap, HashSet};
mod subgraph_verifier;
pub use all_pair_graph::AllPairGraph;
use alloy_primitives::Address;
use brontes_types::pair::Pair;
use itertools::Itertools;
use malachite::Rational;
pub use subgraph_verifier::VerificationResults;
use tracing::info;

use self::{registry::SubGraphRegistry, subgraph::PairSubGraph, subgraph_verifier::*};
use super::PoolUpdate;
use crate::{
    price_graph_types::{PoolPairInfoDirection, SubGraphEdge},
    types::PoolState,
    Protocol,
};

pub struct GraphManager {
    all_pair_graph:     AllPairGraph,
    sub_graph_registry: SubGraphRegistry,
    subgraph_verifier:  SubgraphVerifier,
    /// this is degen but don't want to reorganize all types so that
    /// this struct can hold the db so these closures allow for the wanted
    /// interactions.
    db_load:            Box<dyn Fn(u64, Pair) -> Option<(Pair, Vec<SubGraphEdge>)> + Send + Sync>,
    db_save:            Box<dyn Fn(u64, Pair, Vec<SubGraphEdge>) + Send + Sync>,
}

impl GraphManager {
    pub fn init_from_db_state(
        all_pool_data: HashMap<(Address, Protocol), Pair>,
        sub_graph_registry: HashMap<Pair, Vec<SubGraphEdge>>,
        db_load: Box<dyn Fn(u64, Pair) -> Option<(Pair, Vec<SubGraphEdge>)> + Send + Sync>,
        db_save: Box<dyn Fn(u64, Pair, Vec<SubGraphEdge>) + Send + Sync>,
    ) -> Self {
        let graph = AllPairGraph::init_from_hashmap(all_pool_data);
        let registry = SubGraphRegistry::new(sub_graph_registry);
        let subgraph_verifier = SubgraphVerifier::new();

        Self {
            all_pair_graph: graph,
            sub_graph_registry: registry,
            db_load,
            db_save,
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
        let pair = pair.ordered();
        if let Some((_, edges)) = (&self.db_load)(block, pair) {
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
        self.subgraph_verifier.create_new_subgraph(
            pair.ordered(),
            block,
            edges,
            self.sub_graph_registry.get_edge_state(),
        )
    }

    /// creates a subpool for the pair returning all pools that need to be
    /// loaded
    pub fn create_subgraph_mut(&mut self, block: u64, pair: Pair) -> Vec<PoolPairInfoDirection> {
        let pair = pair.ordered();

        if let Some((pair, edges)) = (&mut self.db_load)(block, pair) {
            return self.subgraph_verifier.create_new_subgraph(
                pair,
                block,
                edges,
                self.sub_graph_registry.get_edge_state(),
            )
        }

        let paths = self
            .all_pair_graph
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

        self.subgraph_verifier.create_new_subgraph(
            pair,
            block,
            paths.clone(),
            self.sub_graph_registry.get_edge_state(),
        )
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

    pub fn add_verified_subgraph(
        &mut self,
        state: HashMap<Address, PoolState>,
        pair: Pair,
        subgraph: PairSubGraph,
    ) {
        self.sub_graph_registry
            .add_verified_subgraph(state, pair.ordered(), subgraph)
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
            .verify_subgraph_on_new_path_failure(pair)
    }

    pub fn get_price(&self, pair: Pair) -> Option<Rational> {
        self.sub_graph_registry.get_price(pair)
    }

    pub fn new_state(&mut self, address: Address, state: PoolState) {
        self.subgraph_verifier.add_edge_state(address, state);
    }

    pub fn update_state(&mut self, address: Address, update: PoolUpdate) {
        self.sub_graph_registry.update_pool_state(address, update);
    }

    pub fn registry_has_state(&self, addr: &Address) -> bool {
        self.sub_graph_registry.has_state(addr)
    }

    pub fn verifier_has_state(&self, block: u64, addr: &Address) -> bool {
        self.subgraph_verifier.has_state(block, addr)
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
        self.subgraph_verifier
            .verify_subgraph(pairs, quote, &self.all_pair_graph)
    }
}
