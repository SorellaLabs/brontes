pub mod mev;
use std::sync::Arc;

use brontes_database::libmdbx::{DBWriter, LibmdbxReader};
use brontes_inspect::Inspector;
use brontes_types::{db::metadata::Metadata, normalized_actions::Actions, tree::BlockTree};
use futures::Future;
pub use mev::*;

pub trait Processor: Send + Sync + 'static + Unpin + Copy + Clone {
    type InspectType: Send + Sync + Unpin;

    fn process_results<DB: DBWriter + LibmdbxReader>(
        db: &DB,
        inspectors: &[&dyn Inspector<Result = Self::InspectType>],
        tree: Arc<BlockTree<Actions>>,
        metadata: Arc<Metadata>,
    ) -> impl Future<Output = ()> + Send;
}
