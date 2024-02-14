mod const_sql;
pub mod db_client;
pub mod dbms;
pub mod errors;
pub use db_client::*;
pub mod http_client;
pub use http_client::*;

#[cfg(feature = "clickhouse-inserts")]
mod middleware;

use std::fmt::Debug;

use brontes_types::db::metadata::Metadata;
use clickhouse::DbRow;
use futures::Future;
use serde::Deserialize;

use crate::{libmdbx::types::LibmdbxData, CompressedTable};

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
