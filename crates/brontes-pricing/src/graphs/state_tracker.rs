use std::ops::RangeInclusive;

use alloy_primitives::Address;
use brontes_types::FastHashMap;
use itertools::Itertools;
use tracing::debug;

use crate::{
    types::{PoolState, PoolUpdate},
    PoolPairInfoDirection, SubGraphEdge,
};

/// Manages the state of pools in the BrontesBatchPricer system, maintaining two
/// types of state data: finalized and verification states.
///
/// `StateTracker` is vital for managing the current state of pools.
/// The tracker holds  finalized states that have been confirmed and are stable,
/// alongside states that are currently under verification.
///
/// The finalized states are used as a reliable foundation for the system's
/// operations, whereas the verification states are used to process new data and
/// updates. The tracker seamlessly handles the transition of states from
/// verification to finalized, ensuring consistency and accuracy in the system's
/// overall functionality.
///
/// Key operations include updating pool states based on new data, managing
/// states under verification, and transitioning states to finalized status upon
/// verification completion. This careful management of pool states is essential
/// for the BrontesBatchPricer system to provide accurate and current pricing
/// information for tokens on decentralized exchanges.
#[derive(Debug, Clone)]
pub struct StateTracker {
    /// state that finalized subgraphs are dependent on.
    finalized_edge_state:    FastHashMap<Address, StateWithDependencies>,
    /// state that verification is using
    verification_edge_state: FastHashMap<Address, PoolStateWithBlock>,
}

impl Drop for StateTracker {
    fn drop(&mut self) {
        let mut ver_byte_cnt = 0usize;
        for (_, p) in &self.verification_edge_state {
            ver_byte_cnt += 8;
            ver_byte_cnt += p.estimate_mem()
        }

        let finalized_byte_cnt = self.finalized_edge_state.len() * 138;

        tracing::info!(
            target: "brontes::mem",
            verification_mem_bytes = ver_byte_cnt,
            finalized_mem_bytes = finalized_byte_cnt,
            "finalized state tracker info"
        );
    }
}

impl Default for StateTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl StateTracker {
    pub fn new() -> Self {
        Self {
            finalized_edge_state:    FastHashMap::default(),
            verification_edge_state: FastHashMap::default(),
        }
    }

    pub fn remove_finalized_state_dep(&mut self, pool: Address, amount: u64) {
        self.finalized_edge_state.retain(|i_pool, state| {
            if pool != *i_pool {
                return true
            }
            state.dec(amount);
            let keep = state.dependents != 0;
            if !keep {
                tracing::debug!(?pool, "removing state");
            }
            keep
        });
    }

    pub fn decrement_verification_state(&mut self, pool: Address, block: u64) {
        if let Some(s) = self
            .verification_edge_state
            .get_mut(&pool)
            .filter(|pool_state| pool_state.contains_block_state(block))
        {
            s.dec_state(block);
        };
    }

    pub fn finalized_state(&self) -> FastHashMap<Address, &PoolState> {
        self.finalized_edge_state
            .iter()
            .map(|(a, d)| (*a, &d.state))
            .collect()
    }

    pub fn all_state(&self, block: u64) -> FastHashMap<Address, &PoolState> {
        self.state_for_verification(block)
            .into_iter()
            .chain(self.finalized_state())
            .collect()
    }

    pub fn all_state_range(&self, block: RangeInclusive<u64>) -> FastHashMap<Address, &PoolState> {
        self.state_for_verification_range(block)
            .into_iter()
            .chain(self.finalized_state())
            .collect()
    }

    pub fn state_for_verification_range(
        &self,
        block: RangeInclusive<u64>,
    ) -> FastHashMap<Address, &PoolState> {
        self.verification_edge_state
            .iter()
            .filter_map(|(addr, state)| Some((*addr, state.get_state_range(&block)?)))
            .collect()
    }

    pub fn state_for_verification(&self, block: u64) -> FastHashMap<Address, &PoolState> {
        self.verification_edge_state
            .iter()
            .filter_map(|(addr, state)| Some((*addr, state.get_state(block)?)))
            .collect()
    }

    pub fn mark_state_as_finalized(&mut self, block: u64, pool: Address) {
        let Some(pool_state) = self.verification_edge_state.get_mut(&pool) else {
            debug!(?pool, "tried to mark a pool that didn't exist as finalized");
            return;
        };

        pool_state.mark_state_as_finalized(block);
    }

    /// will return state that is to be fetched but also will increment state
    /// dep counters
    #[allow(clippy::blocks_in_conditions)]
    pub fn missing_state(
        &mut self,
        block: u64,
        edges: &[SubGraphEdge],
    ) -> Vec<PoolPairInfoDirection> {
        edges
            .iter()
            .filter_map(|edge| {
                if self
                    .verification_edge_state
                    .get_mut(&edge.pool_addr)
                    .filter(|pool_state| pool_state.contains_block_state(block))
                    .map(|s| {
                        s.inc_state(block);
                    })
                    .is_some()
                {
                    return None
                }

                Some(edge.info)
            })
            .collect_vec()
    }

    /// removes all cached state for the given block now that we
    /// have finalized all subgraph creation for this block
    pub fn finalize_block(&mut self, block: u64) {
        self.verification_edge_state
            .iter_mut()
            .for_each(|(pool, state)| {
                let Some((should_finalize, state)) = state.remove_state(block) else {
                    return;
                };

                if should_finalize {
                    match self.finalized_edge_state.entry(*pool) {
                        std::collections::hash_map::Entry::Vacant(v) => {
                            v.insert(state);
                        }
                        std::collections::hash_map::Entry::Occupied(mut o) => {
                            let old_state = o.get_mut();
                            if state.state.last_update > block {
                                panic!("finalized state was ahead of regular state");
                            }
                            old_state.dependents += state.dependents;
                        }
                    }
                }
            });
    }

    pub fn update_pool_state(&mut self, address: Address, update: PoolUpdate) {
        let Some(state) = self.finalized_edge_state.get_mut(&address) else {
            return;
        };

        state.state.increment_state(update);
    }

    pub fn new_state_for_verification(&mut self, address: Address, state: StateWithDependencies) {
        self.verification_edge_state
            .entry(address)
            .or_default()
            .add_state(state);
    }
}

#[derive(Debug, Clone, derive_more::Deref)]
pub struct StateWithDependencies {
    #[deref]
    pub state:      PoolState,
    pub dependents: u64,
}

impl StateWithDependencies {
    pub fn inc(&mut self, am: u64) {
        self.dependents += am;
    }

    pub fn dec(&mut self, am: u64) {
        self.dependents -= am;
    }
}

#[derive(Debug, Default, Clone)]
pub struct PoolStateWithBlock(Vec<(bool, StateWithDependencies)>);

impl PoolStateWithBlock {
    fn estimate_mem(&self) -> usize {
        self.0.len() * 152
    }

    pub fn mark_state_as_finalized(&mut self, block: u64) {
        for (finalized, state) in &mut self.0 {
            if block == state.last_update {
                *finalized = true;
            }
        }
    }

    pub fn inc_state(&mut self, block: u64) {
        if let Some(state) = self
            .0
            .iter_mut()
            .map(|(_, state)| state)
            .find(|state| block == state.last_update)
        {
            state.inc(1);
        }
    }

    pub fn dec_state(&mut self, block: u64) {
        if let Some(state) = self
            .0
            .iter_mut()
            .map(|(_, state)| state)
            .find(|state| block == state.last_update)
        {
            state.dec(1);
        }
    }

    pub fn get_state_range(&self, block: &RangeInclusive<u64>) -> Option<&PoolState> {
        self.0
            .iter()
            .map(|(_, state)| state)
            .find(|&state| block.contains(&state.last_update))
            .map(|state| &state.state)
    }

    pub fn get_state(&self, block: u64) -> Option<&PoolState> {
        self.0
            .iter()
            .map(|(_, state)| state)
            .find(|&state| block == state.last_update)
            .map(|state| &state.state)
    }

    pub fn remove_state(&mut self, block: u64) -> Option<(bool, StateWithDependencies)> {
        let mut res = None;
        self.0.retain(|(keep, state)| {
            if state.last_update == block {
                res = Some((*keep, state.clone()));
                return false
            }
            true
        });

        res
    }

    pub fn add_state(&mut self, state: StateWithDependencies) {
        self.0.push((false, state));
    }

    pub fn contains_block_state(&self, block: u64) -> bool {
        for (_, state) in &self.0 {
            if block == state.last_update {
                return true
            }
        }

        false
    }
}
