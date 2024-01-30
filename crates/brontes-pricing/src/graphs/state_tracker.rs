use std::collections::HashMap;

use alloy_primitives::Address;
use itertools::Itertools;
use tracing::error;

use crate::{
    types::{PoolState, PoolUpdate},
    PoolPairInfoDirection, SubGraphEdge,
};

#[derive(Debug)]
pub struct StateTracker {
    /// state that finalized subgraphs are dependent on.
    finalized_edge_state:    HashMap<Address, PoolState>,
    /// state that verification is using
    verification_edge_state: HashMap<Address, PoolStateWithBlock>,
}

impl StateTracker {
    pub fn new() -> Self {
        Self { finalized_edge_state: HashMap::new(), verification_edge_state: HashMap::new() }
    }

    pub fn finalized_state(&self) -> &HashMap<Address, PoolState> {
        &self.finalized_edge_state
    }

    pub fn state_for_verification(&self, block: u64) -> HashMap<Address, PoolState> {
        self.verification_edge_state
            .iter()
            .filter_map(|(addr, state)| Some((*addr, state.get_state(block)?.clone())))
            .chain(
                self.finalized_edge_state
                    .iter()
                    .filter_map(|(addr, state)| {
                        if state.last_update == block {
                            return Some((*addr, state.clone()))
                        }
                        None
                    }),
            )
            .collect()
    }

    pub fn has_state(&self, block: u64, address: &Address) -> bool {
        self.verification_edge_state
            .get(address)
            .filter(|state| state.contains_block_state(block))
            .map(|_| true)
            .or_else(|| {
                self.finalized_edge_state
                    .get(address)
                    .filter(|state| state.last_update == block)
                    .map(|_| true)
            })
            .is_some()
    }

    pub fn mark_state_as_finalized(&mut self, block: u64, pool: Address) {
        let Some(pool_state) = self.verification_edge_state.get_mut(&pool) else {
            error!(?pool, "tried to mark a pool that didn't exist as finalized");
            return
        };

        pool_state.mark_state_as_finalized(block);
    }

    pub fn missing_state(
        &self,
        block: u64,
        edges: &Vec<SubGraphEdge>,
    ) -> Vec<PoolPairInfoDirection> {
        edges
            .into_iter()
            .filter_map(|edge| {
                self.verification_edge_state
                    .get(&edge.pool_addr)
                    .filter(|pool_state| pool_state.contains_block_state(block))
                    .map(|_| None)
                    .or_else(|| {
                        self.finalized_edge_state
                            .get(&edge.pool_addr)
                            .filter(|state| state.last_update == block)
                            .map(|_| None)
                    })
                    .or_else(|| Some(Some(edge.info)))?
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
                    self.finalized_edge_state.insert(*pool, state);
                }
            });
    }

    pub fn update_pool_state(&mut self, address: Address, update: PoolUpdate) {
        let Some(state) = self.finalized_edge_state.get_mut(&address) else {
            return
        };

        state.increment_state(update);
    }

    pub fn new_state_for_verification(&mut self, address: Address, state: PoolState) {
        self.verification_edge_state
            .entry(address)
            .or_default()
            .add_state(state);
    }
}

#[derive(Debug, Default, Clone)]
pub struct PoolStateWithBlock(Vec<(bool, PoolState)>);

impl PoolStateWithBlock {
    pub fn mark_state_as_finalized(&mut self, block: u64) {
        for (finalized, state) in &mut self.0 {
            if block == state.last_update {
                *finalized = true;
                break
            }
        }
    }

    pub fn get_state(&self, block: u64) -> Option<&PoolState> {
        for (_, state) in &self.0 {
            if block == state.last_update {
                return Some(state)
            }
        }

        None
    }

    pub fn remove_state(&mut self, block: u64) -> Option<(bool, PoolState)> {
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

    pub fn add_state(&mut self, state: PoolState) {
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
