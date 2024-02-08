//! The `DiscoveryInspector` module in `brontes-inspect` specializes in
//! identifying potential MEV transactions. It does this by looking for
//! transactions that are x standard deviations above the average priority fee
//! (where x is the std_dev_threshold paramater, set to 2 by default), or have a
//! coinbase transfer, or are private transactions based on the indexed mempool
//! transactions we have in our metadata database (s/o chainbound).

use std::{collections::HashMap, sync::Arc};

use alloy_primitives::B256;
use brontes_types::{
    mev::{PossibleMev, PossibleMevTriggers},
    normalized_actions::Actions,
    tree::BlockTree,
};

pub struct DiscoveryInspector {
    std_dev_threshold: f64,
}

impl DiscoveryInspector {
    pub fn new(std_dev_threshold: f64) -> Self {
        Self { std_dev_threshold }
    }

    /// Find possible mev transactions in a block tree. This is done by looking
    /// for transactions that are x standard deviations above the average
    /// priority fee, or have a coinbase transfer, or are private transactions.
    pub fn find_possible_mev(&self, tree: Arc<BlockTree<Actions>>) -> HashMap<B256, PossibleMev> {
        let avr_priority = tree.avg_priority_fee;
        let base_fee = tree.header.base_fee_per_gas.unwrap();

        tree.tx_roots
            .iter()
            .enumerate()
            .filter_map(|(tx_idx, root)| {
                let mut triggers = PossibleMevTriggers::default();

                if root.gas_details.priority_fee(base_fee.into()) as f64
                    > avr_priority + (tree.priority_fee_std_dev * self.std_dev_threshold)
                {
                    triggers.high_priority_fee = true;
                }

                if root.is_private() {
                    triggers.is_private = true;
                }

                if root.gas_details.coinbase_transfer.is_some() {
                    triggers.coinbase_transfer = true;
                }

                if triggers.was_triggered() {
                    Some((
                        root.tx_hash,
                        PossibleMev {
                            tx_hash: root.tx_hash,
                            tx_idx: tx_idx.try_into().unwrap(),
                            gas_details: root.gas_details,
                            triggers,
                        },
                    ))
                } else {
                    None
                }
            })
            .collect()
    }
}
