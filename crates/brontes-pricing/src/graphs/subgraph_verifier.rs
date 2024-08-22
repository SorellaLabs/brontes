use std::{pin::Pin, sync::Arc};

use alloy_primitives::Address;
use brontes_types::{execute_on, pair::Pair, FastHashMap, FastHashSet, ToFloatNearest};
use futures::{Future, FutureExt};
use itertools::Itertools;
use malachite::{num::basic::traits::Zero, Rational};
use parking_lot::RwLock;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use tracing::instrument;

use super::{
    state_tracker::StateTracker,
    subgraph::{BadEdge, PairSubGraph, VerificationOutcome},
};
use crate::{
    pending_tasks::PendingHeavyCalcs, types::PairWithFirstPoolHop, AllPairGraph,
    PoolPairInfoDirection, SubGraphEdge,
};

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
#[derive(Debug, Clone)]
pub struct SubgraphVerifier {
    pending_subgraphs:           FastHashMap<PairWithFirstPoolHop, Subgraph>,
    /// because we take the subgraph while where processing we mark it here so
    /// we don't re-create subgraphs.
    processing_subgraph:         FastHashMap<PairWithFirstPoolHop, u64>,
    /// pruned edges of a subgraph that didn't meet liquidity params.
    /// these are stored as in the case we have a subgraph that all critical
    /// edges are below the liq threshold. we want to select the highest liq
    /// pair and thus need to store this information
    subgraph_verification_state: FastHashMap<PairWithFirstPoolHop, SubgraphVerificationState>,
}

impl Drop for SubgraphVerifier {
    fn drop(&mut self) {
        tracing::debug!(
            target: "brontes::mem",
            verification_state_rem = self.subgraph_verification_state.len(),
            pending_subgraph_count = self.pending_subgraphs.len(),
            "amount of remaining state in verifier"
        );
    }
}

impl Default for SubgraphVerifier {
    fn default() -> Self {
        Self::new()
    }
}

impl SubgraphVerifier {
    pub fn new() -> Self {
        Self {
            pending_subgraphs:           FastHashMap::default(),
            processing_subgraph:         FastHashMap::default(),
            subgraph_verification_state: FastHashMap::default(),
        }
    }

    pub fn get_subgraph_extends(&self, pair: PairWithFirstPoolHop) -> Option<Pair> {
        self.pending_subgraphs
            .get(&pair)
            .and_then(|graph| graph.subgraph.extends_to())
    }

    pub fn has_go_through(&self, pair: PairWithFirstPoolHop) -> bool {
        self.pending_subgraphs.contains_key(&pair) || self.processing_subgraph.contains_key(&pair)
    }

    pub fn get_rem_for_block(&self, block: u64) -> Vec<PairWithFirstPoolHop> {
        self.pending_subgraphs
            .iter()
            .filter(|(_, v)| v.block == block)
            .map(|(k, _)| *k)
            .collect()
    }

    pub fn is_done_block(&self, block: u64) -> bool {
        self.pending_subgraphs
            .values()
            .filter(|v| v.block == block)
            .map(|v| v.block)
            .chain(
                self.processing_subgraph
                    .iter()
                    .filter(|(_, b)| **b == block)
                    .map(|(_, b)| *b),
            )
            .count()
            == 0
    }

    pub fn is_verifying_with_block(&self, pair: PairWithFirstPoolHop, block: u64) -> bool {
        self.pending_subgraphs
            .get(&pair)
            .map(|s| s.block == block)
            .or_else(|| self.processing_subgraph.get(&pair).map(|b| *b == block))
            .unwrap_or(false)
    }

    pub fn pool_dep_failure(
        &mut self,
        pair: &PairWithFirstPoolHop,
        pool_addr: Address,
        pool_pair: Pair,
    ) -> bool {
        tracing::debug!(%pair, "dep failure");

        let Some(graph) = self.pending_subgraphs.get_mut(pair) else { return true };
        graph.subgraph.remove_bad_node(pool_pair, pool_addr);

        if graph.subgraph.is_disjoint() {
            self.subgraph_verification_state.remove(pair);
            self.pending_subgraphs.remove(pair);
            return true
        }
        false
    }

    // creates a new subgraph returning
    pub fn create_new_subgraph(
        &mut self,
        pair: PairWithFirstPoolHop,
        extends_to: Option<Pair>,
        block: u64,
        path: Vec<SubGraphEdge>,
        state_tracker: Arc<RwLock<StateTracker>>,
    ) -> Vec<PoolPairInfoDirection> {
        // if we find a subgraph that is the same, we return.
        if self.pending_subgraphs.contains_key(&pair) {
            return vec![]
        };

        let query_state = state_tracker.write().missing_state(block, &path);
        let complete_pair = pair.get_pair();
        let gt = pair.get_goes_through();
        let extend_pair = Pair(complete_pair.0, extends_to.map(|e| e.0).unwrap_or(complete_pair.1));
        let subgraph = PairSubGraph::init(extend_pair, complete_pair, gt, extends_to, path, block);

        if self
            .pending_subgraphs
            .insert(
                pair,
                Subgraph {
                    subgraph,
                    block,
                    frayed_end_extensions: FastHashMap::default(),
                    id: 0,
                    in_rundown: false,
                    iters: 0,
                },
            )
            .is_some()
        {
            tracing::error!(?pair, ?block, "duplicate subgraph");
        };

        query_state
    }

    #[instrument(skip_all, level = "debug")]
    #[instrument(skip(self), level = "trace")]
    pub fn verify_subgraph_on_new_path_failure(
        &mut self,
        pair: PairWithFirstPoolHop,
    ) -> Option<Vec<Pair>> {
        self.pending_subgraphs
            .get_mut(&pair)
            .or_else(|| {
                tracing::warn!(?pair, "missing pending subgraph");
                None
            })?
            .in_rundown = true;

        let state = self
            .subgraph_verification_state
            .get_mut(&pair)
            .or_else(|| {
                tracing::debug!(?pair, "missing state");
                None
            })?;

        Some(state.sorted_ignore_nodes_by_liquidity())
    }

    fn store_edges_with_liq(
        &mut self,
        pair: PairWithFirstPoolHop,
        removals: &FastHashMap<Pair, FastHashSet<BadEdge>>,
        all_graph: Arc<RwLock<AllPairGraph>>,
    ) {
        removals
            .iter()
            .filter_map(|(k, v)| {
                // look for edges that have been completely removed
                if all_graph.read().edge_count(k.0, k.1) == v.len() {
                    Some(
                        v.clone()
                            .into_iter()
                            .filter(|v| v.liquidity != Rational::ZERO),
                    )
                } else {
                    None
                }
            })
            .flatten()
            .for_each(|edge| {
                let state = self.subgraph_verification_state.entry(pair).or_default();
                state.add_edge_with_liq(edge.pair.0, edge.clone());
                state.add_edge_with_liq(edge.pair.1, edge.clone());
            });
    }

    pub fn add_frayed_end_extension(
        &mut self,
        pair: PairWithFirstPoolHop,
        block: u64,
        state_tracker: Arc<RwLock<StateTracker>>,
        frayed_end_extensions: Vec<SubGraphEdge>,
    ) -> Option<(Vec<PoolPairInfoDirection>, u64, bool)> {
        Some((
            state_tracker
                .write()
                .missing_state(block, &frayed_end_extensions),
            self.pending_subgraphs
                .get_mut(&pair)
                .or_else(|| {
                    tracing::trace!("frayed ext no pair in pending_subgraphs");
                    None
                })?
                .add_extension(frayed_end_extensions),
            true,
        ))
    }

    // async time
    pub fn start_verify_subgraph(
        &mut self,
        pair: Vec<(u64, Option<u64>, PairWithFirstPoolHop, Rational, Address)>,
        state_tracker: Arc<RwLock<StateTracker>>,
    ) -> Pin<Box<dyn Future<Output = PendingHeavyCalcs> + Send>> {
        let pairs = self.get_subgraphs(pair);

        execute_on!(async_pricing, {
            PendingHeavyCalcs::SubgraphVerification(Self::verify_par(pairs, state_tracker.clone()))
        })
        .boxed()
    }

    pub fn verify_subgraph_finish(
        &mut self,
        args: Vec<(PairWithFirstPoolHop, u64, VerificationOutcome, Subgraph)>,
        all_graph: Arc<RwLock<AllPairGraph>>,
        state_tracker: Arc<RwLock<StateTracker>>,
    ) -> Vec<VerificationResults> {
        args.into_iter()
            .map(|(pair, block, result, subgraph)| {
                self.store_edges_with_liq(pair, &result.removals, all_graph.clone());

                // state that we want to be ignored on the next graph search.
                let state = self.subgraph_verification_state.entry(pair).or_default();

                let ignores = state.get_nodes_to_ignore();

                //  all results that should be pruned from our main graph.
                let removals = result
                    .removals
                    .clone()
                    .into_iter()
                    .filter(|(k, _)| !(ignores.contains(k)))
                    .collect::<FastHashMap<_, _>>();

                self.processing_subgraph.remove(&pair);

                if result.should_abandon {
                    self.subgraph_verification_state.remove(&pair);
                    tracing::trace!(?pair, "aborting");
                    return VerificationResults::Abort(pair, block)
                }

                if result.should_requery {
                    let extends = subgraph.subgraph.extends_to();
                    self.pending_subgraphs.insert(pair, subgraph);
                    // anything that was fully remove gets cached
                    tracing::trace!(?pair, "requerying");

                    return VerificationResults::Failed(VerificationFailed {
                        pair,
                        extends,
                        block,
                        prune_state: removals,
                        ignore_state: ignores,
                        frayed_ends: result.frayed_ends,
                    })
                }

                self.passed_verification(pair, block, subgraph, removals, state_tracker.clone())
            })
            .collect_vec()
    }

    fn get_subgraphs(
        &mut self,
        pair: Vec<(u64, Option<u64>, PairWithFirstPoolHop, Rational, Address)>,
    ) -> Vec<(PairWithFirstPoolHop, u64, bool, Subgraph, Rational, Address)> {
        pair.into_iter()
            .map(|(block, frayed, pair, price, quote)| {
                (
                    pair,
                    block,
                    frayed,
                    self.pending_subgraphs.remove(&pair).or_else(|| {
                        tracing::warn!(?pair, "not found in pending subgraphs");
                        None
                    }),
                    price,
                    quote,
                )
            })
            .filter_map(|(pair, block, _, subgraph, price, quote)| {
                self.processing_subgraph.insert(pair, block);
                let mut subgraph = subgraph?;
                subgraph.iters += 1;

                Some((pair, block, subgraph.in_rundown, subgraph, price, quote))
            })
            .collect_vec()
    }

    fn verify_par(
        pairs: Vec<(PairWithFirstPoolHop, u64, bool, Subgraph, Rational, Address)>,
        state_tracker: Arc<RwLock<StateTracker>>,
    ) -> Vec<(PairWithFirstPoolHop, u64, VerificationOutcome, Subgraph)> {
        pairs
            .into_par_iter()
            .map(|(pair, block, rundown, mut subgraph, price, quote)| {
                let r = state_tracker.read();
                let edge_state = r.state_for_verification(block);
                let result = if rundown {
                    subgraph
                        .subgraph
                        .rundown_subgraph_check(quote, price, &edge_state)
                } else {
                    subgraph.subgraph.verify_subgraph(quote, price, edge_state)
                };

                (pair, block, result, subgraph)
            })
            .collect::<Vec<_>>()
    }

    fn passed_verification(
        &mut self,
        pair: PairWithFirstPoolHop,
        block: u64,
        subgraph: Subgraph,
        removals: FastHashMap<Pair, FastHashSet<BadEdge>>,
        state_tracker: Arc<RwLock<StateTracker>>,
    ) -> VerificationResults {
        self.subgraph_verification_state.remove(&pair);
        // remove state for pair
        // mark used state finalized
        let subgraph = subgraph.subgraph;
        subgraph.get_all_pools().flatten().for_each(|pool| {
            state_tracker
                .write()
                .mark_state_as_finalized(block, pool.pool_addr);
        });

        VerificationResults::Passed(VerificationPass {
            pair,
            block,
            subgraph,
            prune_state: removals,
        })
    }
}

#[derive(Debug, Clone)]
pub struct Subgraph {
    pub subgraph:              PairSubGraph,
    pub frayed_end_extensions: FastHashMap<u64, Vec<SubGraphEdge>>,
    pub id:                    u64,
    pub in_rundown:            bool,
    pub iters:                 usize,
    pub block:                 u64,
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
    pub pair:        PairWithFirstPoolHop,
    pub block:       u64,
    pub subgraph:    PairSubGraph,
    pub prune_state: FastHashMap<Pair, FastHashSet<BadEdge>>,
}
#[derive(Debug)]
pub struct VerificationFailed {
    pub pair:         PairWithFirstPoolHop,
    pub extends:      Option<Pair>,
    pub block:        u64,
    // prunes the partial edges of this state.
    pub prune_state:  FastHashMap<Pair, FastHashSet<BadEdge>>,
    // the state that should be ignored when we re-query.
    pub ignore_state: FastHashSet<Pair>,
    // ends that we were able to get to before disjointness occurred
    pub frayed_ends:  Vec<Address>,
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub enum VerificationResults {
    Passed(VerificationPass),
    Failed(VerificationFailed),
    Abort(PairWithFirstPoolHop, u64),
}

#[derive(Debug, Default, Clone)]
pub struct SubgraphVerificationState {
    /// contains all fully removed edges. this is so that
    /// if we don't find a edge with the wanted amount of liquidity,
    /// we can lookup the edge with the best liquidity.
    edges:            EdgesWithLiq,
    /// when we are recusing we remove most liquidity edges until we find a
    /// proper path. However we want to make sure on recusion that these
    /// don't get removed
    removed_recusing: FastHashMap<Pair, Address>,
}

impl SubgraphVerificationState {
    /// returns pairs to ignore from lowest to highest liquidity.
    fn sorted_ignore_nodes_by_liquidity(&self) -> Vec<Pair> {
        self.edges
            .0
            .values()
            .flat_map(|node| {
                node.iter()
                    .map(|n| (n.pair, n.liquidity.clone()))
                    .collect_vec()
            })
            .unique()
            .sorted_by(|a, b| a.1.cmp(&b.1))
            .map(|n| n.0)
            .collect_vec()
    }

    #[allow(unused)]
    fn highest_liq_for_pair(&self, pair: Pair) -> (Address, f64) {
        self.edges
            .0
            .values()
            .flat_map(|node| {
                node.iter()
                    .map(|n| (n.pair, n.pool_address, n.liquidity.clone()))
                    .collect_vec()
            })
            .unique()
            .filter(|f| f.0 == pair)
            .sorted_by(|a, b| a.2.cmp(&b.2))
            .collect_vec()
            .pop()
            .map(|(_, addr, liq)| (addr, liq.to_float()))
            .unwrap()
    }

    fn add_edge_with_liq(&mut self, addr: Address, bad_edge: BadEdge) {
        if !self.removed_recusing.contains_key(&bad_edge.pair) {
            self.edges.0.entry(addr).or_default().insert(bad_edge);
        }
    }

    /// Grabs all the nodes that we want the graph search to ignore
    fn get_nodes_to_ignore(&self) -> FastHashSet<Pair> {
        self.edges
            .0
            .values()
            .flatten()
            .map(|node| node.pair.ordered())
            .collect::<FastHashSet<_>>()
    }
}

#[derive(Debug, Default, Clone)]
pub struct EdgesWithLiq(FastHashMap<Address, FastHashSet<BadEdge>>);
