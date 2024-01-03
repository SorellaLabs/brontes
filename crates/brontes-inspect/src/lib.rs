pub mod atomic_backrun;
pub mod cex_dex;
pub mod composer;
pub mod jit;
#[allow(dead_code, unused_imports)]
pub mod liquidations;
pub mod sandwich;
pub mod shared_utils;

use std::sync::Arc;

use brontes_database::Metadata;
use brontes_types::{
    classified_mev::{ClassifiedMev, SpecificMev},
    normalized_actions::Actions,
    tree::TimeTree,
};

#[async_trait::async_trait]
pub trait Inspector: Send + Sync {
    async fn process_tree(
        &self,
        tree: Arc<TimeTree<Actions>>,
        metadata: Arc<Metadata>,
    ) -> Vec<(ClassifiedMev, Box<dyn SpecificMev>)>;
}
