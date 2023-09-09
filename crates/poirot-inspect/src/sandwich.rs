use super::database::InspectorDataClient;
use crate::Inspector;
use poirot_types::{normalized_actions::Actions, tree::TimeTree};
use reth_primitives::H256;
use std::{collections::VecDeque, sync::Arc};

pub struct SandwichInspector {
    db: Arc<InspectorDataClient>,
}

#[async_trait::async_trait]
impl Inspector for SandwichInspector {
    async fn process_tree(&self, tree: Arc<TimeTree<Actions>>) {
        let mut hashes: VecDeque<H256> = tree.get_hashes().into();
        while hashes.len() > 2 {
            hashes.pop_front();
        }
    }
}
