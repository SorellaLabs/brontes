use std::collections::{hash_map::Entry, HashMap, HashSet};

use alloy_primitives::Address;
use brontes_types::{pair::Pair, unzip_either::IterExt};
use itertools::Itertools;
use rayon::iter::{IntoParallelIterator, ParallelIterator};

use super::subgraph::{BadEdge, PairSubGraph, VerificationOutcome};
use crate::{types::PoolState, AllPairGraph, PoolPairInfoDirection, SubGraphEdge};

#[derive(Debug)]
pub struct SubgraphVerifier {
    pending_subgraphs:           HashMap<Pair, PairSubGraph>,
    /// pruned edges of a subgraph that didn't meet liquidity params.
    /// these are stored as in the case we have a subgraph that all critical
    /// edges are below the liq threshold. we want to select the highest liq
    /// pair and thus need to store this information
    subgraph_verification_state: HashMap<Pair, SubgraphVerificationState>,
    /// The state of a edge that is needed for verification. Because its
    /// possible to be verifying for multiple blocks at once that use the
    /// same pool, we need to also track blocks of the state
    edge_state:                  HashMap<Address, PoolStateWithBlock>,
    /// counts how many of the factory pairs are using the edge.
    state_deps:                  HashMap<Address, PoolStateTracker>,
}

impl SubgraphVerifier {
    pub fn new() -> Self {
        return Self {
            edge_state:                  HashMap::new(),
            pending_subgraphs:           HashMap::new(),
            subgraph_verification_state: HashMap::new(),
            state_deps:                  HashMap::new(),
        }
    }

    pub fn all_pairs(&self) -> Vec<Pair> {
        self.pending_subgraphs.keys().copied().collect_vec()
    }

    pub fn is_verifying(&self, pair: &Pair) -> bool {
        self.pending_subgraphs.contains_key(pair)
    }

    pub fn has_state(&self, block: u64, addr: &Address) -> bool {
        self.edge_state
            .get(addr)
            .map(|i| i.contains_block_state(block))
            .unwrap_or(false)
    }

    pub fn add_edge_state(&mut self, address: Address, state: PoolState) {
        self.edge_state.entry(address).or_default().add_state(state);
    }

    // creates a new subgraph returning
    pub fn create_new_subgraph(
        &mut self,
        pair: Pair,
        block: u64,
        path: Vec<SubGraphEdge>,
        registry_state: &HashMap<Address, PoolState>,
    ) -> Vec<PoolPairInfoDirection> {
        // TODO: clean the logic up here
        let (query_state, registry_state_to_use): (Vec<_>, Vec<_>) = path
            .iter()
            .map(|e| {
                // we will have this entry regardless
                self.state_deps
                    .entry(e.pool_addr)
                    .or_default()
                    .increment_block(block);

                if let Some(entries) = self.edge_state.get(&e.pool_addr) {
                    if entries.contains_block_state(block) {
                        return (None, None)
                    // check if registry has what we want
                    } else {
                        let Some(state) = registry_state.get(&e.pool_addr) else {
                            return (Some(e.info.clone()), None)
                        };

                        if state.last_update == block {
                            return (None, Some((e.pool_addr, state.clone())))
                        } else {
                            return (Some(e.info.clone()), None)
                        }
                    }
                } else if let Some(state) = registry_state.get(&e.pool_addr) {
                    if state.last_update == block {
                        return (None, Some((e.pool_addr, state.clone())))
                    } else {
                        return (Some(e.info.clone()), None)
                    }
                } else {
                    (Some(e.info.clone()), None)
                }
            })
            .unzip_either();

        registry_state_to_use
            .into_iter()
            .for_each(|(addr, pool)| self.edge_state.entry(addr).or_default().add_state(pool));

        // init subgraph
        let subgraph = PairSubGraph::init(pair.ordered(), path);
        self.pending_subgraphs.insert(pair.ordered(), subgraph);

        query_state
    }

    /// this isn't the most optimal. However will do for now
    pub fn verify_subgraph_on_new_path_failure(&mut self, pair: Pair) -> Option<Vec<Pair>> {
        let state = self.subgraph_verification_state.get_mut(&pair)?;
        Some(state.sorted_ignore_nodes_by_liquidity())
    }

    pub fn verify_subgraph(
        &mut self,
        pair: Vec<(u64, Pair)>,
        quote: Address,
        all_graph: &AllPairGraph,
    ) -> Vec<VerificationResults> {
        let pairs = self.get_subgraphs(pair);
        let res = self.verify_par(pairs, quote, all_graph);

        res.into_iter()
            .map(|(pair, block, result, subgraph)| {
                result
                    .removals
                    .iter()
                    .filter_map(|(k, v)| {
                        // look for edges that have been complety removed
                        if all_graph.edge_count(k.0, k.1) == v.len() {
                            Some((*k, v.clone()))
                        } else {
                            None
                        }
                    })
                    .for_each(|(_, bad_state)| {
                        // cache all edges that have been completey removed
                        for edge in bad_state {
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
                        }
                    });

                // process only edge state.
                self.subgraph_verification_state
                    .entry(pair)
                    .or_default()
                    .process_only_edge_state(result.was_only_edge_state);

                // state that we want to be ignored on the next graph search.
                let ignores = self
                    .subgraph_verification_state
                    .entry(pair)
                    .or_default()
                    .get_nodes_to_ignore();

                // don't remove something that we want to ignore
                let removals = result
                    .removals
                    .into_iter()
                    .filter(|(k, _)| !ignores.contains(k))
                    .collect::<HashMap<_, _>>();

                // remove removal state from tracking
                removals.iter().for_each(|(_, edges)| {
                    for edge in edges {
                        if let Some(block) = self
                            .state_deps
                            .get_mut(&edge.pool_address)
                            .and_then(|state| state.decrement_block(block))
                        {
                            if let Entry::Occupied(mut o) = self.edge_state.entry(edge.pool_address)
                            {
                                if o.get_mut().remove_state(block).is_none() {
                                    return
                                }

                                if o.get().is_empty() {
                                    o.remove_entry();
                                }
                            }
                        }
                    }
                });

                if result.should_requery {
                    self.pending_subgraphs.insert(pair.ordered(), subgraph);
                    // anything that was fully remove gets cached
                    return VerificationResults::Failed(VerificationFailed {
                        pair,
                        block,
                        prune_state: removals,
                        ignore_state: ignores,
                    })
                }

                self.passed_verification(pair, block, subgraph, removals)
            })
            .collect_vec()
    }

    fn get_subgraphs(&mut self, pair: Vec<(u64, Pair)>) -> Vec<(Pair, u64, PairSubGraph)> {
        pair.into_iter()
            .map(|(block, pair)| (pair, block, self.pending_subgraphs.remove(&pair.ordered())))
            .filter_map(|(pair, block, subgraph)| {
                let Some(subgraph) = subgraph else { return None };

                Some((pair.ordered(), block, subgraph))
            })
            .collect_vec()
    }

    fn verify_par(
        &self,
        pairs: Vec<(Pair, u64, PairSubGraph)>,
        quote: Address,
        all_graph: &AllPairGraph,
    ) -> Vec<(Pair, u64, VerificationOutcome, PairSubGraph)> {
        pairs
            .into_par_iter()
            .map(|(pair, block, mut subgraph)| {
                let edge_state = self
                    .edge_state
                    .iter()
                    .filter_map(|(k, inner)| {
                        let last_state = inner.get_state(block)?;
                        Some((*k, last_state.clone()))
                    })
                    .collect::<HashMap<_, _>>();

                let default = SubgraphVerificationState::default();

                let result = subgraph.verify_subgraph(
                    quote,
                    &edge_state,
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
                        .get_nodes_to_ignore(),
                );

                (pair, block, result, subgraph)
            })
            .collect::<Vec<_>>()
    }

    fn passed_verification(
        &mut self,
        pair: Pair,
        block: u64,
        subgraph: PairSubGraph,
        removals: HashMap<Pair, HashSet<BadEdge>>,
    ) -> VerificationResults {
        // remove cached pruned edges
        self.subgraph_verification_state.remove(&pair).map(|state| {
            state
                .edges
                .0
                .into_values()
                .flat_map(|edge| edge.into_iter())
                .filter(|edge| !state.best_edge_nodes.contains_key(&edge.pair))
                .unique()
                .for_each(|edge| {
                    if let Entry::Occupied(mut o) = self.state_deps.entry(edge.pool_address) {
                        let state_tracker = o.get_mut();
                        if state_tracker.decrement_block(block).is_none() {
                            return
                        }
                        if let Entry::Occupied(mut o) = self.edge_state.entry(edge.pool_address) {
                            o.get_mut().remove_state(block);
                            if o.get().is_empty() {
                                o.remove_entry();
                            }
                        }
                    }
                });
        });

        // remove state relevant to finalized subgraph
        let edge_state = subgraph
            .get_all_pools()
            .flatten()
            .filter_map(|edge| {
                let pair = Pair(edge.token_1, edge.token_0).ordered();
                if let Entry::Occupied(mut o) = self.state_deps.entry(edge.pool_addr) {
                    let state_tracker = o.get_mut();
                    if let Some(remove_block) = state_tracker.decrement_block(block) {
                        let state =
                            if let Entry::Occupied(mut o) = self.edge_state.entry(edge.pool_addr) {
                                let entry = o.get_mut();

                                let state = entry.remove_state(remove_block).unwrap();
                                if entry.is_empty() {
                                    o.remove_entry();
                                }
                                state
                            } else {
                                unreachable!();
                            };
                        Some((pair, edge.pool_addr, state))
                    } else {
                        Some((
                            pair,
                            edge.pool_addr,
                            self.edge_state
                                .get(&edge.pool_addr)
                                .unwrap()
                                .get_state(block)
                                .unwrap()
                                .clone(),
                        ))
                    }
                } else {
                    tracing::error!(?edge.pool_addr, "no deps found for addr");
                    None
                }
            })
            .filter(|(pair, addr, _)| {
                if let Some(inner) = removals.get(&pair) {
                    for i in inner {
                        if i.pool_address == *addr {
                            return false
                        }
                    }
                }
                true
            })
            .map(|(_, a, b)| (a, b))
            .collect::<HashMap<_, _>>();

        VerificationResults::Passed(VerificationPass {
            pair,
            state: edge_state,
            subgraph,
            prune_state: removals,
        })
    }
}

#[derive(Debug)]
pub struct VerificationPass {
    pub pair:        Pair,
    pub subgraph:    PairSubGraph,
    // state for block thats needed.
    pub state:       HashMap<Address, PoolState>,
    pub prune_state: HashMap<Pair, HashSet<BadEdge>>,
}
#[derive(Debug)]
pub struct VerificationFailed {
    pub pair:         Pair,
    pub block:        u64,
    // prunes the partial edges of this state.
    pub prune_state:  HashMap<Pair, HashSet<BadEdge>>,
    // the state that should be ignored when we requery.
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
    edges:           EdgesWithLiq,
    /// graph edge to the pair that we allow for low liqudity price calcs.
    /// this is stored seperate as it is possible to have multiple iterations
    /// where we have more than one path hop that is low liquidity.
    best_edge_nodes: HashMap<Pair, Address>,
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
    fn get_nodes_to_ignore(&self) -> HashSet<Pair> {
        self.edges
            .0
            .values()
            .filter_map(|node| {
                let n = node.first()?;

                (!self.best_edge_nodes.contains_key(&n.pair)).then(|| n.pair)
            })
            .collect()
    }

    /// takes the edge state that is isolated, check for other paths from
    /// the given edge and then set the pair that has the max liqudity
    fn process_only_edge_state(&mut self, state: HashSet<Address>) {
        state.into_iter().for_each(|addr| {
            if let Some(best_edge) = self.edges.max_liq_for_edge(&addr) {
                self.best_edge_nodes
                    .insert(best_edge.pair, best_edge.pool_address);
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

#[derive(Debug, Default, Clone)]
struct PoolStateWithBlock(Vec<PoolState>);

impl PoolStateWithBlock {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn get_state(&self, block: u64) -> Option<&PoolState> {
        for state in &self.0 {
            if block == state.last_update {
                return Some(state)
            }
        }

        None
    }

    pub fn remove_state(&mut self, block: u64) -> Option<PoolState> {
        let mut res = None;
        self.0.retain(|state| {
            if state.last_update == block {
                res = Some(state.clone());
                return false
            }
            true
        });

        res
    }

    pub fn add_state(&mut self, state: PoolState) {
        self.0.push(state);
    }

    pub fn contains_block_state(&self, block: u64) -> bool {
        for state in &self.0 {
            if block == state.last_update {
                return true
            }
        }

        false
    }
}

#[derive(Debug, Default, Clone)]
struct PoolStateTracker(Vec<(u16, u64)>);

impl PoolStateTracker {
    fn increment_block(&mut self, block: u64) {
        let mut last_cnt = None;
        for (cnt, block_number) in &mut self.0 {
            if block == *block_number {
                last_cnt = Some(cnt);
                break
            }
        }

        if let Some(cnt) = last_cnt {
            *cnt += 1
        } else {
            self.0.push((1, block));
        }
    }

    /// returns a block if it was the last subgraph using the state
    fn decrement_block(&mut self, block: u64) -> Option<u64> {
        let mut last_cnt = None;
        for (cnt, block_number) in &mut self.0 {
            if block == *block_number {
                last_cnt = Some((cnt, *block_number));
                break
            }
        }

        if let Some((cnt, remove_bn)) = last_cnt {
            if *cnt == 1 {
                self.0.retain(|(_, bn)| *bn != remove_bn);
                return Some(remove_bn)
            } else {
                *cnt -= 1;
            }
        }

        None
    }
}
