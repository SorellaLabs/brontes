pub mod atomic_backrun;
pub mod sandwich;

use std::sync::Arc;

use clickhouse::Row;
use poirot_labeller::Metadata;
use poirot_types::{normalized_actions::Actions, tree::TimeTree};
use reth_primitives::H256;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Row)]
pub struct ClassifiedMev {
    pub tx_hash:           H256,
    // gas related
    pub gas_used:          u64,
    pub gas_bribe:         u64,
    pub coinbase_transfer: Option<u64>
}

#[async_trait::async_trait]
pub trait Inspector: Send + Sync {
    async fn process_tree(
        &self,
        tree: Arc<TimeTree<Actions>>,
        metadata: Arc<Metadata>
    ) -> Vec<ClassifiedMev>;
}
