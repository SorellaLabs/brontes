use std::collections::{HashMap, HashSet};

use alloy_primitives::Address;
use brontes_types::pair::Pair;
use itertools::Itertools;
use rayon::iter::{IntoParallelIterator, ParallelIterator};

use super::{
    state_tracker::StateTracker,
    subgraph::{BadEdge, PairSubGraph, VerificationOutcome},
};
use crate::{AllPairGraph, PoolPairInfoDirection, SubGraphEdge};

/// [`SubgraphVerifier`] Manages the verification of subgraphs for token pairs
/// in the BrontesBatchPricer system. It ensures the accuracy and relevance of
/// subgraphs, which are essential for pricing tokens on DEXs.
///
/// The struct performs several critical functions:
///
/// - `pending_subgraphs`: Maintains a collection of subgraphs currently
///   undergoing verification. These represent token pairs and are crucial for
///   calculating accurate prices.
///
/// - `subgraph_verification_state`: Tracks the state of subgraphs during the
///   verification process. It includes information on pruned edges that did not
///   meet liquidity parameters, helping to select edges with the highest
///   liquidity in case of critical edges falling below the threshold.
///
/// - `create_new_subgraph`: Generates new subgraphs for specific token pairs,
///   adding them to the pending list for verification. This method is key in
///   determining the relevant parts of the token graph for a pair.
///
/// - `verify_subgraph`: Verifies subgraphs to ensure they accurately reflect
///   the current state of the DEX, checking liquidity parameters and pool
///   states. This method is vital in maintaining the integrity of the pricing
///   system.
#[derive(Debug)]
pub struct SubgraphVerifier {
    pending_subgraphs:           HashMap<Pair, Subgraph>,
    /// pruned edges of a subgraph that didn't meet liquidity params.
    /// these are stored as in the case we have a subgraph that all critical
    /// edges are below the liq threshold. we want to select the highest liq
    /// pair and thus need to store this information
    subgraph_verification_state: HashMap<Pair, SubgraphVerificationState>,
}

impl SubgraphVerifier {
    pub fn new() -> Self {
        return Self {
            pending_subgraphs:           HashMap::new(),
            subgraph_verification_state: HashMap::new(),
        }
    }

    pub fn all_pairs(&self) -> Vec<Pair> {
        self.pending_subgraphs.keys().copied().collect_vec()
    }

    pub fn is_verifying(&self, pair: &Pair) -> bool {
        self.pending_subgraphs.contains_key(pair)
    }

    // creates a new subgraph returning
    pub fn create_new_subgraph(
        &mut self,
        pair: Pair,
        block: u64,
        path: Vec<SubGraphEdge>,
        state_tracker: &StateTracker,
    ) -> Vec<PoolPairInfoDirection> {
        let query_state = state_tracker.missing_state(block, &path);

        let subgraph = PairSubGraph::init(pair, path);
        if self.pending_subgraphs.contains_key(&pair) {
            return vec![]
        };

        self.pending_subgraphs
            .insert(pair, Subgraph { subgraph, frayed_end_extensions: HashMap::new(), id: 0 });

        query_state
    }

    pub fn verify_subgraph_on_new_path_failure(&mut self, pair: Pair) -> Option<Vec<Pair>> {
        let state = self.subgraph_verification_state.get_mut(&pair)?;
        Some(state.sorted_ignore_nodes_by_liquidity())
    }

    fn store_edges_with_liq(
        &mut self,
        pair: Pair,
        removals: &HashMap<Pair, HashSet<BadEdge>>,
        all_graph: &AllPairGraph,
    ) {
        removals
            .into_iter()
            .flat_map(|(k, v)| v.into_iter())
            .for_each(|edge| {
                // cache all edges that have been completey removed
                self.subgraph_verification_state
                    .entry(pair)
                    .or_default()
                    .add_edge_with_liq(edge.pair.0, edge.clone());

                self.subgraph_verification_state
                    .entry(pair)
                    .or_default()
                    .add_edge_with_liq(edge.pair.1, edge.clone());
            });
    }

    pub fn add_frayed_end_extension(
        &mut self,
        pair: Pair,
        block: u64,
        state_tracker: &StateTracker,
        frayed_end_extensions: Vec<SubGraphEdge>,
    ) -> Option<(Vec<PoolPairInfoDirection>, u64)> {
        Some((
            state_tracker.missing_state(block, &frayed_end_extensions),
            self.pending_subgraphs
                .get_mut(&pair)?
                .add_extension(frayed_end_extensions),
        ))
    }

    pub fn verify_subgraph(
        &mut self,
        pair: Vec<(u64, Option<u64>, Pair)>,
        quote: Address,
        all_graph: &AllPairGraph,
        state_tracker: &mut StateTracker,
    ) -> Vec<VerificationResults> {
        let pairs = self.get_subgraphs(pair);
        let res = self.verify_par(pairs, quote, all_graph, state_tracker);

        res.into_iter()
            .map(|(pair, block, result, subgraph)| {
                self.store_edges_with_liq(pair, &result.removals, all_graph);

                // state that we want to be ignored on the next graph search.
                let ignores = self
                    .subgraph_verification_state
                    .entry(pair)
                    .or_default()
                    .get_nodes_to_ignore();
                //
                // // all results that should be pruned from our main graph.
                // let removals = result
                //     .removals
                //     .clone()
                //     .into_iter()
                //     .filter(|(k, _)| !(ignores.contains(k)))
                //     .collect::<HashMap<_, _>>();
                //
                //

                if result.should_requery {
                    self.pending_subgraphs.insert(pair, subgraph);
                    // anything that was fully remove gets cached

                    tracing::info!(?pair, "requerying");

                    return VerificationResults::Failed(VerificationFailed {
                        pair,
                        block,
                        prune_state: HashMap::new(),
                        ignore_state: ignores,
                        frayed_ends: result.frayed_ends,
                    })
                }

                self.passed_verification(pair, block, subgraph, HashMap::new(), state_tracker)
            })
            .collect_vec()
    }

    fn get_subgraphs(&mut self, pair: Vec<(u64, Option<u64>, Pair)>) -> Vec<(Pair, u64, Subgraph)> {
        pair.into_iter()
            .map(|(block, frayed, pair)| {
                (pair, block, frayed, self.pending_subgraphs.remove(&pair))
            })
            .filter_map(|(pair, block, frayed, subgraph)| {
                let Some(mut subgraph) = subgraph else { return None };
                if let Some(frayed) = frayed {
                    let extensions = subgraph.frayed_end_extensions.remove(&frayed).unwrap();
                    subgraph.subgraph.extend_subgraph(extensions);
                }

                Some((pair, block, subgraph))
            })
            .collect_vec()
    }

    fn verify_par(
        &self,
        pairs: Vec<(Pair, u64, Subgraph)>,
        quote: Address,
        all_graph: &AllPairGraph,
        state_tracker: &mut StateTracker,
    ) -> Vec<(Pair, u64, VerificationOutcome, Subgraph)> {
        pairs
            .into_par_iter()
            .map(|(pair, block, mut subgraph)| {
                let edge_state = state_tracker.state_for_verification(block);
                let result = subgraph
                    .subgraph
                    .verify_subgraph(quote, edge_state, all_graph);

                (pair, block, result, subgraph)
            })
            .collect::<Vec<_>>()
    }

    fn passed_verification(
        &mut self,
        pair: Pair,
        block: u64,
        subgraph: Subgraph,
        removals: HashMap<Pair, HashSet<BadEdge>>,
        state_tracker: &mut StateTracker,
    ) -> VerificationResults {
        // remove state for pair
        let _ = self.subgraph_verification_state.remove(&pair);
        // mark used state finalized
        let subgraph = subgraph.subgraph;
        subgraph.get_all_pools().flatten().for_each(|pool| {
            state_tracker.mark_state_as_finalized(block, pool.pool_addr);
        });

        VerificationResults::Passed(VerificationPass { pair, subgraph, prune_state: removals })
    }
}

#[derive(Debug)]
pub struct Subgraph {
    pub subgraph:              PairSubGraph,
    pub frayed_end_extensions: HashMap<u64, Vec<SubGraphEdge>>,
    pub id:                    u64,
}
impl Subgraph {
    pub fn add_extension(&mut self, edges: Vec<SubGraphEdge>) -> u64 {
        let id = self.id;
        self.id += 1;
        self.frayed_end_extensions.insert(id, edges);

        id
    }
}

#[derive(Debug)]
pub struct VerificationPass {
    pub pair:        Pair,
    pub subgraph:    PairSubGraph,
    pub prune_state: HashMap<Pair, HashSet<BadEdge>>,
}
#[derive(Debug)]
pub struct VerificationFailed {
    pub pair:         Pair,
    pub block:        u64,
    // prunes the partial edges of this state.
    pub prune_state:  HashMap<Pair, HashSet<BadEdge>>,
    // the state that should be ignored when we re-query.
    pub ignore_state: HashSet<Pair>,
    // ends that we were able to get to before disjointness occurred
    pub frayed_ends:  Vec<Address>,
}

#[derive(Debug)]
pub enum VerificationResults {
    Passed(VerificationPass),
    Failed(VerificationFailed),
}

impl VerificationResults {
    pub fn split(self) -> (Option<VerificationPass>, Option<VerificationFailed>) {
        match self {
            Self::Passed(p) => (Some(p), None),
            Self::Failed(f) => (None, Some(f)),
        }
    }
}

#[derive(Debug, Default)]
pub struct SubgraphVerificationState {
    /// contains all fully removed edges. this is so that
    /// if we don't find a edge with the wanted amount of liquidity,
    /// we can lookup the edge with the best liquidity.
    edges:            EdgesWithLiq,
    /// when we are recusing we remove most liquidity edges until we find a
    /// proper path. However we want to make sure on recusion that these
    /// don't get removed
    removed_recusing: HashMap<Pair, Address>,
}

impl SubgraphVerificationState {
    /// returns pairs to ignore from highest to lowest liquidity.
    fn sorted_ignore_nodes_by_liquidity(&self) -> Vec<Pair> {
        self.edges
            .0
            .values()
            .flat_map(|node| {
                node.into_iter()
                    .map(|n| (n.pair, n.liquidity.clone()))
                    .collect_vec()
            })
            .unique()
            .sorted_by(|a, b| a.1.cmp(&b.1))
            .map(|n| n.0)
            .collect_vec()
    }

    fn add_edge_with_liq(&mut self, addr: Address, bad_edge: BadEdge) {
        if !self.removed_recusing.contains_key(&bad_edge.pair) {
            self.edges.0.entry(addr).or_default().insert(bad_edge);
        }
    }

    fn get_recusing_nodes(&self) -> &HashMap<Pair, Address> {
        &self.removed_recusing
    }

    /// Grabs all the nodes that we want the graph search to ignore
    fn get_nodes_to_ignore(&self) -> HashSet<Pair> {
        self.edges
            .0
            .values()
            .flatten()
            .map(|node| node.pair.ordered())
            .collect::<HashSet<_>>()
    }
}

#[derive(Debug, Default)]
pub struct EdgesWithLiq(HashMap<Address, HashSet<BadEdge>>);
