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

#[derive(Debug)]
pub struct SubgraphVerifier {
    pending_subgraphs:           HashMap<Pair, PairSubGraph>,
    /// pruned edges of a subgraph that didn't meet liquidity params.
    /// these are stored as in the case we have a subgraph that all critical
    /// edges are below the liq threshold. we want to select the highest liq
    /// pair and thus need to store this information
    subgraph_verification_state: HashMap<Pair, SubgraphVerificationState>,
    /// holds all the edges that we want to remove after a block has been
    /// completed. we wait till the block has been completed as sometimes we
    /// can remove a node that is actually critical to another pair.
    pools_to_remove:             HashMap<u64, HashMap<Pair, HashSet<BadEdge>>>,
}

impl SubgraphVerifier {
    pub fn new() -> Self {
        return Self {
            pending_subgraphs:           HashMap::new(),
            subgraph_verification_state: HashMap::new(),
            pools_to_remove:             HashMap::new(),
        }
    }

    pub fn finalize_block(
        &mut self,
        block: u64,
        state_tracker: &StateTracker,
    ) -> HashMap<Pair, HashSet<BadEdge>> {
        let Some(data) = self.pools_to_remove.remove(&block) else {
            return HashMap::new();
        };

        // current state being verified
        let do_not_remove = self
            .subgraph_verification_state
            .iter()
            .flat_map(|(_, state)| state.get_complete_edges_to_ignore())
            .collect::<HashSet<_>>();

        data.into_iter()
            .filter(|(pair, _)| !do_not_remove.contains(&pair))
            .map(|(pair, pools)| {
                (
                    pair,
                    pools
                        .into_iter()
                        .filter(|pool| !state_tracker.has_finalized_state(&pool.pool_address))
                        .collect(),
                )
            })
            .collect()
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
        self.pending_subgraphs.insert(pair.ordered(), subgraph);

        query_state
    }

    /// this isn't the most optimal. However will do for now
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
            .filter_map(|(k, v)| {
                // look for edges that have been complety removed
                if all_graph.edge_count(k.0, k.1) == v.len() {
                    Some(v.clone().into_iter())
                } else {
                    None
                }
            })
            .flatten()
            .for_each(|edge| {
                // cache all edges that have been completey removed
                self.subgraph_verification_state
                    .entry(pair.ordered())
                    .or_default()
                    .edges
                    .add_edge_with_liq(edge.pair.0, edge.clone());

                self.subgraph_verification_state
                    .entry(pair.ordered())
                    .or_default()
                    .edges
                    .add_edge_with_liq(edge.pair.1, edge.clone());
            });
    }

    pub fn verify_subgraph(
        &mut self,
        pair: Vec<(u64, Pair)>,
        quote: Address,
        all_graph: &AllPairGraph,
        state_tracker: &mut StateTracker,
    ) -> Vec<VerificationResults> {
        let pairs = self.get_subgraphs(pair);
        let res = self.verify_par(pairs, quote, all_graph, state_tracker);

        res.into_iter()
            .map(|(pair, block, result, subgraph)| {
                // store all edges with there liquidity if there the only pool for the pair.
                self.store_edges_with_liq(pair, &result.removals, all_graph);

                // mark edges that are the only edge in the graph
                self.subgraph_verification_state
                    .entry(pair)
                    .or_default()
                    .process_only_edge_state(result.was_only_edge_state);

                // state that we want to be ignored on the next graph search.
                let ignores = self
                    .subgraph_verification_state
                    .entry(pair)
                    .or_default()
                    .get_complete_edges_to_ignore();

                // all results that should be pruned from our main graph.
                let removals = result
                    .removals
                    .into_iter()
                    .filter(|(k, _)| !ignores.contains(k))
                    .collect::<HashMap<_, _>>();

                self.subgraph_verification_state
                    .entry(pair)
                    .or_default()
                    .state_to_remove(removals);

                if result.should_requery {
                    self.pending_subgraphs.insert(pair.ordered(), subgraph);
                    // anything that was fully remove gets cached

                    tracing::info!(?pair, "requerying ignoring: {} ", ignores.len());
                    return VerificationResults::Failed(VerificationFailed {
                        pair,
                        block,
                        ignore_state: ignores,
                    })
                }

                self.passed_verification(pair, block, subgraph, state_tracker)
            })
            .collect_vec()
    }

    fn get_subgraphs(&mut self, pair: Vec<(u64, Pair)>) -> Vec<(Pair, u64, PairSubGraph)> {
        pair.into_iter()
            .map(|(block, pair)| (pair, block, self.pending_subgraphs.remove(&pair.ordered())))
            .filter_map(|(pair, block, subgraph)| {
                let Some(subgraph) = subgraph else { return None };

                Some((pair, block, subgraph))
            })
            .collect_vec()
    }

    fn verify_par(
        &self,
        pairs: Vec<(Pair, u64, PairSubGraph)>,
        quote: Address,
        all_graph: &AllPairGraph,
        state_tracker: &mut StateTracker,
    ) -> Vec<(Pair, u64, VerificationOutcome, PairSubGraph)> {
        pairs
            .into_par_iter()
            .map(|(pair, block, mut subgraph)| {
                let edge_state = state_tracker.state_for_verification(block);
                let default = SubgraphVerificationState::default();

                let result = subgraph.verify_subgraph(
                    quote,
                    edge_state,
                    all_graph,
                    &self
                        .subgraph_verification_state
                        .get(&pair)
                        .unwrap_or(&default)
                        .best_edge_nodes,
                    &self
                        .subgraph_verification_state
                        .get(&pair)
                        .unwrap_or(&default)
                        .get_complete_edges_to_ignore(),
                    &self
                        .subgraph_verification_state
                        .get(&pair)
                        .unwrap_or(&default)
                        .get_pools_to_ignore(),
                );

                (pair, block, result, subgraph)
            })
            .collect::<Vec<_>>()
    }

    fn passed_verification(
        &mut self,
        pair: Pair,
        block: u64,
        mut subgraph: PairSubGraph,
        state_tracker: &mut StateTracker,
    ) -> VerificationResults {
        // remove state for pair
        if let Some(state) = self.subgraph_verification_state.remove(&pair) {
            // remove all state from subgraph that we have flagged
            state
                .possible_removal_state
                .iter()
                .for_each(|(_, edges)| {
                    for edge in edges {
                        assert!(!subgraph.remove_bad_node(edge.pair, edge.pool_address));
                    }
                });

            self.pools_to_remove
                .entry(block)
                .or_default()
                .extend(state.possible_removal_state)
        }
        // mark used state finalized
        subgraph.get_all_pools().flatten().for_each(|pool| {
            state_tracker.mark_state_as_finalized(block, pool.pool_addr);
        });

        VerificationResults::Passed(VerificationPass { pair, subgraph })
    }
}

#[derive(Debug)]
pub struct VerificationPass {
    pub pair:     Pair,
    pub subgraph: PairSubGraph,
}
#[derive(Debug)]
pub struct VerificationFailed {
    pub pair:         Pair,
    pub block:        u64,
    // the state that should be ignored when we re-query.
    pub ignore_state: HashSet<Pair>,
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
    edges:                  EdgesWithLiq,
    possible_removal_state: HashMap<Pair, HashSet<BadEdge>>,
    /// graph edge to the pair that we allow for low liquidity price calcs.
    /// this is stored seperate as it is possible to have multiple iterations
    /// where we have more than one path hop that is low liquidity.
    best_edge_nodes:        HashMap<Pair, Address>,
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
            .sorted_by(|a, b| a.1.cmp(&b.1))
            .map(|n| n.0.ordered())
            .collect_vec()
    }

    /// Grabs all the nodes that we want the graph search to ignore
    fn get_complete_edges_to_ignore(&self) -> HashSet<Pair> {
        self.edges
            .0
            .values()
            .flatten()
            .filter_map(|node| {
                (!self.best_edge_nodes.contains_key(&node.pair.ordered())).then(|| node.pair)
            })
            .collect()
    }

    fn state_to_remove(&mut self, state: HashMap<Pair, HashSet<BadEdge>>) {
        state.into_iter().for_each(|(pair, edges)| {
            self.possible_removal_state
                .entry(pair)
                .or_default()
                .extend(edges);
        });
    }

    // pools that we have pruned
    fn get_pools_to_ignore(&self) -> HashSet<Address> {
        self.possible_removal_state
            .values()
            .flatten()
            .map(|e| e.pool_address)
            .collect()
    }

    /// takes the edge state that is isolated, check for other paths from
    /// the given edge and then set the pair that has the max liquidity
    fn process_only_edge_state(&mut self, state: HashSet<Address>) {
        state.into_iter().for_each(|addr| {
            if let Some(best_edge) = self.edges.max_liq_for_edge(&addr) {
                self.best_edge_nodes
                    .insert(best_edge.pair.ordered(), best_edge.pool_address);
            }
        });
    }
}

#[derive(Debug, Default)]
pub struct EdgesWithLiq(HashMap<Address, Vec<BadEdge>>);

impl EdgesWithLiq {
    fn max_liq_for_edge(&self, addr: &Address) -> Option<BadEdge> {
        self.0.get(addr).and_then(|values| {
            values
                .into_iter()
                .max_by(|a, b| a.liquidity.cmp(&b.liquidity))
                .cloned()
        })
    }

    fn add_edge_with_liq(&mut self, addr: Address, bad_edge: BadEdge) {
        self.0.entry(addr).or_default().push(bad_edge);
    }
}
