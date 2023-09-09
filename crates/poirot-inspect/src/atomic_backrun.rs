use crate::{database::InspectorDataClient, Inspector};
use poirot_types::{normalized_actions::Actions, tree::TimeTree};
use std::sync::Arc;

pub struct AtomicBackrunInspector {
    db: Arc<InspectorDataClient>,
}

impl AtomicBackrunInspector {}

#[async_trait::async_trait]
impl Inspector for AtomicBackrunInspector {
    async fn process_tree(&self, tree: Arc<TimeTree<Actions>>) {
        let intersting_state = tree.inspect_all(|node| node.data.is_swap());
    }
}
