// #[cfg(feature = "local-postgres")]
// mod const_sql;
#[cfg(feature = "local-postgres")]
pub mod db_client;
#[cfg(feature = "local-postgres")]
pub mod dbms;
pub mod errors;
#[cfg(feature = "local-postgres")]
pub use db_client::*;
// #[cfg(feature = "local-postgres")]
// pub mod split_db;
#[cfg(feature = "local-postgres")]
pub use db_interfaces::postgres::config::PostgresConfig;
// #[cfg(feature = "local-postgres")]
// pub use split_db::*;
#[cfg(feature = "local-postgres")]
pub mod types;
#[cfg(not(feature = "local-postgres"))]
pub mod http_client;
#[cfg(not(feature = "local-postgres"))]
pub use http_client::*;

// #[cfg(feature = "local-postgres")]
// mod middleware;
use std::fmt::Debug;

pub mod cex_config;

use ::clickhouse::DbRow;
use brontes_types::db::metadata::Metadata;
// #[cfg(feature = "local-postgres")]
// pub use const_sql::*;
#[cfg(feature = "local-postgres")]
use db_interfaces::postgres::client::PostgresClient;
#[cfg(feature = "local-postgres")]
pub use dbms::BrontesPostgresTables;
use futures::Future;
// #[cfg(feature = "local-postgres")]
// pub use middleware::*;
use serde::Deserialize;

use crate::{
    libmdbx::{cex_utils::CexRangeOrArbitrary, types::LibmdbxData},
    CompressedTable,
};

#[auto_impl::auto_impl(&, &mut)]
pub trait PostgresHandle: Send + Sync + Unpin + 'static {
    fn get_metadata(&self, block_num: u64) -> impl Future<Output = eyre::Result<Metadata>> + Send;

    fn get_cex_prices(
        &self,
        range_or_arbitrary: CexRangeOrArbitrary,
    ) -> impl Future<Output = eyre::Result<Vec<crate::CexPriceData>>> + Send;

    fn get_cex_trades(
        &self,
        range_or_arbitrary: CexRangeOrArbitrary,
    ) -> impl Future<Output = eyre::Result<Vec<crate::CexTradesData>>> + Send;

    fn query_many_range<T, D>(
        &self,
        start_block: u64,
        end_block: u64,
    ) -> impl Future<Output = eyre::Result<Vec<D>>> + Send
    where
        T: CompressedTable,
        T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
        D: LibmdbxData<T> + DbRow + for<'de> Deserialize<'de> + Send + Debug + Unpin + 'static;

    fn query_many_arbitrary<T, D>(
        &self,
        range: &'static [u64],
    ) -> impl Future<Output = eyre::Result<Vec<D>>> + Send
    where
        T: CompressedTable,
        T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
        D: LibmdbxData<T> + DbRow + for<'de> Deserialize<'de> + Send + Debug + Unpin + 'static;

    fn query_many<T, D>(&self) -> impl Future<Output = eyre::Result<Vec<D>>> + Send
    where
        T: CompressedTable,
        T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
        D: LibmdbxData<T> + DbRow + for<'de> Deserialize<'de> + Send + Debug + Unpin + 'static;

    #[cfg(feature = "local-postgres")]
    fn inner(&self) -> &PostgresClient<BrontesPostgresTables>;

    #[cfg(feature = "local-postgres")]
    fn get_init_crit_tables(
        &self,
    ) -> impl Future<Output = eyre::Result<PostgresCritTableCount>> + Send;
}
