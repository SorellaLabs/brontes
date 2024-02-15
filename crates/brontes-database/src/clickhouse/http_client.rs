use std::fmt::Debug;

use ::clickhouse::DbRow;
use brontes_types::db::metadata::Metadata;
use serde::Deserialize;

use crate::{clickhouse::ClickhouseHandle, libmdbx::types::LibmdbxData, CompressedTable};

#[allow(dead_code)]
pub struct ClickhouseHttpClient(reqwest::Client);

impl ClickhouseHttpClient {
    pub fn new(_url: String, _api_key: String) -> Self {
        todo!()
    }
}

impl ClickhouseHandle for ClickhouseHttpClient {
    async fn get_metadata(&self, _block_num: u64) -> eyre::Result<Metadata> {
        todo!()
    }

    async fn query_many_range<T, D>(
        &self,
        _start_block: u64,
        _end_block: u64,
    ) -> eyre::Result<Vec<D>>
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
