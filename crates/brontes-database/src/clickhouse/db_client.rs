use std::fmt::Debug;

use alloy_primitives::Address;
use brontes_types::{
    constants::{USDT_ADDRESS, WETH_ADDRESS},
    db::{
        cex::CexPriceMap,
        clickhouse::*,
        dex::{DexQuote, DexQuotes},
        metadata::Metadata,
        searcher::SearcherInfo,
        token_info::{TokenInfo, TokenInfoWithAddress},
    },
    mev::{Bundle, BundleData, Mev, MevBlock},
    pair::Pair,
    structured_trace::{TxTrace, TxTraces},
    Protocol,
};
use clickhouse::DbRow;
use serde::Deserialize;
use sorella_db_databases::{
    clickhouse::{
        config::ClickhouseConfig, db::ClickhouseClient, utils::format_query_array, Credentials,
    },
    tables::{DatabaseTables, DexTokens},
    Database,
};

use super::{
    const_sql::*,
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
    async fn write_searcher_info(
        &self,
        searcher_eoa: Address,
        searcher_info: SearcherInfo,
    ) -> eyre::Result<()> {
        self.client
            .insert_one::<ClickhouseSearcherInfo>(&searcher_info)
            .await?;

        Ok(())
    }

    async fn save_mev_blocks(
        &self,
        block_number: u64,
        block: MevBlock,
        mev: Vec<Bundle>,
    ) -> eyre::Result<()> {
        self.client
            .insert_one::<ClickhouseMevBlocks>(&block)
            .await?;
        Ok(())
    }

    async fn write_dex_quotes(
        &self,
        block_num: u64,
        quotes: Option<DexQuotes>,
    ) -> eyre::Result<()> {
        if let Some(quotes) = quotes {
            self.client
                .insert_one::<ClickhouseDexQuotes>(&quotes)
                .await?;
        }

        Ok(())
    }

    async fn write_token_info(
        &self,
        address: Address,
        decimals: u8,
        symbol: String,
    ) -> eyre::Result<()> {
        // self.client
        //     .insert_one::<DBTokenInfo>(&TokenInfoWithAddress {
        //         address,
        //         inner: TokenInfo { symbol, decimals },
        //     })
        //     .await?;

        Ok(())
    }

    async fn insert_pool(
        &self,
        block: u64,
        address: Address,
        tokens: [Address; 2],
        classifier_name: Protocol,
    ) -> eyre::Result<()> {
        Ok(())
    }

    async fn save_traces(&self, block: u64, traces: Vec<TxTrace>) -> eyre::Result<()> {
        self.client
            .insert_one::<ClickhouseTxTraces>(&(traces.into()))
            .await?;

        Ok(())
    }
}

impl ClickhouseHandle for Clickhouse {
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
