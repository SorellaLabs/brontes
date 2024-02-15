mod const_sql;
#[cfg(feature = "local-clickhouse")]
pub mod db_client;
#[cfg(feature = "local-clickhouse")]
pub mod dbms;
pub mod errors;
#[cfg(feature = "local-clickhouse")]
pub use db_client::*;
#[cfg(not(feature = "local-clickhouse"))]
pub mod http_client;
#[cfg(not(feature = "local-clickhouse"))]
pub use http_client::*;

#[cfg(feature = "local-clickhouse")]
mod middleware;
use std::fmt::Debug;

use brontes_types::db::metadata::Metadata;
use clickhouse::DbRow;
#[cfg(feature = "local-clickhouse")]
pub use const_sql::*;
use futures::Future;
#[cfg(feature = "local-clickhouse")]
pub use middleware::*;
use serde::Deserialize;

use crate::{libmdbx::types::LibmdbxData, CompressedTable};

#[auto_impl::auto_impl(&, &mut)]
pub trait ClickhouseHandle: Send + Sync + Unpin + 'static {
    fn get_metadata(
        &self,
        block_num: u64,
    ) -> impl Future<Output = eyre::Result<Metadata>> + Send + Sync;

    fn query_many_range<T, D>(
        &self,
        start_block: u64,
        end_block: u64,
    ) -> impl Future<Output = eyre::Result<Vec<D>>> + Send + Sync
    where
        T: CompressedTable,
        T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
        D: LibmdbxData<T> + DbRow + for<'de> Deserialize<'de> + Send + Sync + Debug + 'static;

    fn query_many<T, D>(&self) -> impl Future<Output = eyre::Result<Vec<D>>> + Send + Sync
    where
        T: CompressedTable,
        T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
        D: LibmdbxData<T> + DbRow + for<'de> Deserialize<'de> + Send + Sync + Debug + 'static;
}
