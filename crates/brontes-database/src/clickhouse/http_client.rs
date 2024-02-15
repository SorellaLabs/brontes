use std::fmt::Debug;

use brontes_types::db::metadata::Metadata;
use clickhouse::DbRow;
use serde::Deserialize;

use crate::{clickhouse::ClickhouseHandle, libmdbx::types::LibmdbxData, CompressedTable};

pub struct ClickhouseHttpClient(reqwest::Client);

impl ClickhouseHttpClient {
    pub fn new(url: String, api_key: String) -> Self {
        todo!()
    }
}

impl ClickhouseHandle for ClickhouseHttpClient {
    async fn get_metadata(&self, block_num: u64) -> eyre::Result<Metadata> {
        todo!()
    }

    async fn query_many_range<T, D>(&self, start_block: u64, end_block: u64) -> eyre::Result<Vec<D>>
    where
        T: CompressedTable,
        T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
        D: LibmdbxData<T> + DbRow + for<'de> Deserialize<'de> + Send + Sync + Debug + 'static,
    {
        todo!()
    }

    async fn query_many<T, D>(&self) -> eyre::Result<Vec<D>>
    where
        T: CompressedTable,
        T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
        D: LibmdbxData<T> + DbRow + for<'de> Deserialize<'de> + Send + Sync + Debug + 'static,
    {
        todo!()
    }
}
