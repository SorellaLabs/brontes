use super::database::InspectorDataClient;
use crate::Inspector;

use poirot_types::{normalized_actions::Actions, tree::TimeTree};
use std::sync::Arc;

pub struct SandwichInspector {
    db: Arc<InspectorDataClient>,
}

#[async_trait::async_trait]
impl Inspector for SandwichInspector {
    async fn process_tree(&self, tree: Arc<TimeTree<Actions>>) {}
}
