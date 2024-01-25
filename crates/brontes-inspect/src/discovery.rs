use std::{collections::HashMap, sync::Arc};

use alloy_primitives::B256;
use brontes_types::{
    classified_mev::{PossibleMev, PossibleMevTriggers},
    normalized_actions::Actions,
    tree::BlockTree,
};

/// DiscoveryInspector classifies Possible transactions that we would want to be
/// discovered. The discovery inspector is always ran
pub struct DiscoveryInspector {
    priority_deviation: u128,
}

impl DiscoveryInspector {
    pub fn new(priority_deviation: u128) -> Self {
        Self { priority_deviation }
    }

    pub fn find_possible_mev(&self, tree: Arc<BlockTree<Actions>>) -> HashMap<B256, PossibleMev> {
        let avr_priority = tree.avg_priority_fee;
        let base_fee = tree.header.base_fee_per_gas.unwrap();

        tree.tx_roots
            .iter()
            .enumerate()
            .map(|(tx_idx, root)| {
                let mut triggers = PossibleMevTriggers::default();

                if root.gas_details.priority_fee(base_fee.into())
                    > avr_priority * self.priority_deviation
                {
                    triggers.high_priority_fee = true;
                }

                if root.is_private() {
                    triggers.is_private = true;
                }

                if root.gas_details.coinbase_transfer.is_some() {
                    triggers.coinbase_transfer = true;
                }
                PossibleMev {
                    tx_hash: root.tx_hash,
                    tx_idx: tx_idx.try_into().unwrap(),
                    gas_details: root.gas_details.clone(),
                    triggers,
                }
            })
            .filter(|possible| possible.triggers.was_triggered())
            .map(|possible| (possible.tx_hash, possible))
            .collect()
    }
}
