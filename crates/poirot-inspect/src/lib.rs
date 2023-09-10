pub mod atomic_backrun;
pub mod sandwich;

use std::sync::Arc;

use poirot_labeller::Labeller;
use poirot_types::{normalized_actions::Actions, tree::TimeTree};

#[async_trait::async_trait]
pub trait Inspector: Send + Sync {
    async fn process_tree(&self, tree: Arc<TimeTree<Actions>>);
}
