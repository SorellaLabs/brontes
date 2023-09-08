use crate::{database::InspectorDataClient, Inspector};
use poirot_types::{normalized_actions::Actions, tree::TimeTree};
use std::sync::Arc;

pub struct BackrunInspector {
    db: Arc<InspectorDataClient>,
}

#[async_trait::async_trait]
impl Inspector for BackrunInspector {
    async fn process_tree(&self, tree: Arc<TimeTree<Actions>>) {}
}
