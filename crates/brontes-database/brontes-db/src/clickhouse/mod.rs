#[cfg(feature = "local-clickhouse")]
mod const_sql;
#[cfg(feature = "local-clickhouse")]
pub mod db_client;
#[cfg(feature = "local-clickhouse")]
pub mod dbms;
pub mod errors;
#[cfg(feature = "local-clickhouse")]
pub use db_client::*;
#[cfg(feature = "local-clickhouse")]
pub mod split_db;
#[cfg(feature = "local-clickhouse")]
pub use db_interfaces::clickhouse::config::ClickhouseConfig;
use reth_primitives::{Address, BlockHash, TxHash};
#[cfg(feature = "local-clickhouse")]
pub use split_db::*;
#[cfg(not(feature = "local-clickhouse"))]
pub mod http_client;
#[cfg(not(feature = "local-clickhouse"))]
pub use http_client::*;

#[cfg(feature = "local-clickhouse")]
mod middleware;
use std::fmt::Debug;

pub mod cex_config;

use ::clickhouse::DbRow;
use brontes_types::db::metadata::Metadata;
#[cfg(feature = "local-clickhouse")]
pub use const_sql::*;
#[cfg(feature = "local-clickhouse")]
use db_interfaces::clickhouse::client::ClickhouseClient;
#[cfg(feature = "local-clickhouse")]
pub use dbms::BrontesClickhouseTables;
use futures::Future;
#[cfg(feature = "local-clickhouse")]
pub use middleware::*;
use serde::Deserialize;

use crate::{
    libmdbx::{cex_utils::CexRangeOrArbitrary, types::LibmdbxData},
    CompressedTable,
};

#[auto_impl::auto_impl(&, &mut)]
pub trait ClickhouseHandle: Send + Sync + Unpin + 'static {
    fn get_metadata(
        &self,
        block_num: u64,
        block_hash: BlockHash,
        tx_hashes_in_block: Vec<TxHash>,
        quote_asset: Address,
    ) -> impl Future<Output = eyre::Result<Metadata>> + Send;

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

    #[cfg(feature = "local-clickhouse")]
    fn inner(&self) -> &ClickhouseClient<BrontesClickhouseTables>;

    #[cfg(feature = "local-clickhouse")]
    fn get_init_crit_tables(
        &self,
    ) -> impl Future<Output = eyre::Result<ClickhouseCritTableCount>> + Send;
}
