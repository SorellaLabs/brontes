pub mod mev;
use std::{panic::AssertUnwindSafe, sync::Arc};

use brontes_database::libmdbx::{DBWriter, LibmdbxReader};
use brontes_inspect::Inspector;
use brontes_types::{
    db::metadata::Metadata, normalized_actions::Actions, tree::BlockTree, BrontesTaskExecutor,
};
use futures::{Future, FutureExt};
pub use mev::*;
use tracing::{span, Instrument, Level};

pub trait Processor: Send + Sync + 'static + Unpin + Copy + Clone {
    type InspectType: Send + Sync + Unpin;

    fn process_results_inner<DB: DBWriter + LibmdbxReader>(
        db: &DB,
        inspectors: &[&dyn Inspector<Result = Self::InspectType>],
        tree: Arc<BlockTree<Actions>>,
        metadata: Arc<Metadata>,
    ) -> impl Future<Output = ()> + Send;

    fn process_results<DB: DBWriter + LibmdbxReader>(
        db: &DB,
        inspectors: &[&dyn Inspector<Result = Self::InspectType>],
        tree: Arc<BlockTree<Actions>>,
        metadata: Arc<Metadata>,
    ) -> impl Future<Output = ()> + Send {
        let block_number = metadata.block_num;
        async move {
            if let Err(e) =
                AssertUnwindSafe(Self::process_results_inner(db, inspectors, tree, metadata))
                    .catch_unwind()
                    .in_current_span()
                    .await
            {
                let error = e.downcast_ref::<String>().cloned().unwrap_or(
                    e.downcast_ref::<&str>()
                        .map(|s| (*s).to_string())
                        .unwrap_or_default(),
                );

                tracing::error!(error=%error, "hit panic while processing results");
                BrontesTaskExecutor::current().trigger_shutdown("processor");
            }
        }
        .instrument(span!(Level::ERROR, "mev processor", %block_number))
    }
}
