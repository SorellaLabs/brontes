pub mod mev;

use brontes_database::libmdbx::{DBWriter, LibmdbxReader};
use brontes_inspect::Inspector;
use brontes_types::MultiBlockData;
use futures::Future;
pub use mev::*;

pub trait Processor: Send + Sync + 'static + Unpin + Copy + Clone {
    type InspectType: Send + Sync + Unpin;

    fn process_results<DB: DBWriter + LibmdbxReader>(
        db: &'static DB,
        inspectors: &'static [&dyn Inspector<Result = Self::InspectType>],
        data: MultiBlockData,
    ) -> impl Future<Output = ()> + Send;
}
