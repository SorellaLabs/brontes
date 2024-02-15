use std::fmt::Debug;

use ::clickhouse::DbRow;
use alloy_primitives::Address;
use brontes_types::{
    db::{dex::DexQuotes, metadata::Metadata, searcher::SearcherInfo},
    mev::{Bundle, MevBlock},
    structured_trace::TxTrace,
    Protocol,
};
use serde::Deserialize;
use sorella_db_databases::{
    clickhouse::{config::ClickhouseConfig, db::ClickhouseClient},
    Database,
};

use super::{
    dbms::{
        BrontesClickhouseTables, ClickhouseDexQuotes, ClickhouseMevBlocks, ClickhouseSearcherInfo,
        ClickhouseTxTraces,
    },
    ClickhouseHandle,
};
use crate::{libmdbx::types::LibmdbxData, CompressedTable};

#[derive(Default)]
pub struct Clickhouse {
    client: ClickhouseClient<BrontesClickhouseTables>,
}

impl Clickhouse {
    pub fn new(config: ClickhouseConfig) -> Self {
        let client = ClickhouseClient::new(config);
        Self { client }
    }

    pub fn inner(&self) -> &ClickhouseClient<BrontesClickhouseTables> {
        &self.client
    }

    // inserts
    pub async fn write_searcher_info(
        &self,
        _searcher_eoa: Address,
        searcher_info: SearcherInfo,
    ) -> eyre::Result<()> {
        self.client
            .insert_one::<ClickhouseSearcherInfo>(&searcher_info)
            .await?;

        Ok(())
    }

    pub async fn save_mev_blocks(
        &self,
        _block_number: u64,
        block: MevBlock,
        _mev: Vec<Bundle>,
    ) -> eyre::Result<()> {
        self.client
            .insert_one::<ClickhouseMevBlocks>(&block)
            .await?;
        Ok(())
    }

    pub async fn write_dex_quotes(
        &self,
        _block_num: u64,
        quotes: Option<DexQuotes>,
    ) -> eyre::Result<()> {
        if let Some(quotes) = quotes {
            self.client
                .insert_one::<ClickhouseDexQuotes>(&quotes)
                .await?;
        }

        Ok(())
    }

    pub async fn write_token_info(
        &self,
        _address: Address,
        _decimals: u8,
        _symbol: String,
    ) -> eyre::Result<()> {
        // self.client
        //     .insert_one::<DBTokenInfo>(&TokenInfoWithAddress {
        //         address,
        //         inner: TokenInfo { symbol, decimals },
        //     })
        //     .await?;

        Ok(())
    }

    pub async fn insert_pool(
        &self,
        _block: u64,
        _address: Address,
        _tokens: [Address; 2],
        _classifier_name: Protocol,
    ) -> eyre::Result<()> {
        Ok(())
    }

    pub async fn save_traces(&self, _block: u64, traces: Vec<TxTrace>) -> eyre::Result<()> {
        self.client
            .insert_one::<ClickhouseTxTraces>(&(traces.into()))
            .await?;

        Ok(())
    }
}

impl ClickhouseHandle for Clickhouse {
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
        D: LibmdbxData<T> + DbRow + for<'de> Deserialize<'de> + Send + Debug + 'static,
    {
        todo!()
    }

    async fn query_many<T, D>(&self) -> eyre::Result<Vec<D>>
    where
        T: CompressedTable,
        T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
        D: LibmdbxData<T> + DbRow + for<'de> Deserialize<'de> + Send + Debug + 'static,
    {
        todo!()
    }

    fn inner(&self) -> &ClickhouseClient<BrontesClickhouseTables> {
        &self.client
    }
}
