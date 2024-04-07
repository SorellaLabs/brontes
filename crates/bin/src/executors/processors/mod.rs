pub mod mev;

use brontes_database::libmdbx::{DBWriter, LibmdbxReader};
use brontes_inspect::Inspector;
use brontes_types::{
    db::metadata::Metadata, mev::events::Action, normalized_actions::Actions, tree::BlockTree,
};
use futures::Future;
pub use mev::*;
//tui related
use tokio::sync::mpsc::UnboundedSender;

pub trait Processor: Send + Sync + 'static + Unpin + Copy + Clone {
    type InspectType: Send + Sync + Unpin;

    fn process_results<DB: DBWriter + LibmdbxReader>(
        db: &DB,
        inspectors: &[&dyn Inspector<Result = Self::InspectType>],
        tree: Arc<BlockTree<Actions>>,
        metadata: Arc<Metadata>,
        tui_tx: UnboundedSender<Action>,
    ) -> impl Future<Output = ()> + Send;
}
