use std::{cmp::max, fmt::Debug};

use ::clickhouse::DbRow;
use alloy_primitives::Address;
use brontes_types::{
    db::{
        dex::DexQuotes,
        metadata::{BlockMetadata, Metadata},
        searcher::SearcherInfo,
    },
    mev::{Bundle, MevBlock},
    structured_trace::TxTrace,
    Protocol,
};
use serde::Deserialize;
use sorella_db_databases::{
    clickhouse::{config::ClickhouseConfig, db::ClickhouseClient},
    Database,
};

use super::{dbms::BrontesClickhouseTables, ClickhouseHandle};
use crate::{
    clickhouse::const_sql::{BLOCK_INFO, CEX_PRICE},
    libmdbx::{
        determine_eth_prices,
        tables::{BlockInfoData, CexPriceData},
        types::LibmdbxData,
    },
    CompressedTable,
};

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
        _searcher_info: SearcherInfo,
    ) -> eyre::Result<()> {
        // self.client
        //     .insert_one::<ClickhouseSearcherInfo>(&searcher_info)
        //     .await?;

        Ok(())
    }

    pub async fn save_mev_blocks(
        &self,
        _block_number: u64,
        _block: MevBlock,
        _mev: Vec<Bundle>,
    ) -> eyre::Result<()> {
        // self.client
        //     .insert_one::<ClickhouseMevBlocks>(&block)
        //     .await?;
        Ok(())
    }

    pub async fn write_dex_quotes(
        &self,
        _block_num: u64,
        _quotes: Option<DexQuotes>,
    ) -> eyre::Result<()> {
        // if let Some(quotes) = quotes {
        //     self.client
        //         .insert_one::<ClickhouseDexQuotes>(&quotes)
        //         .await?;
        // }

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

    pub async fn save_traces(&self, _block: u64, _traces: Vec<TxTrace>) -> eyre::Result<()> {
        // self.client
        //     .insert_one::<ClickhouseTxTraces>(&(traces.into()))
        //     .await?;

        Ok(())
    }
}

impl ClickhouseHandle for Clickhouse {
    async fn get_metadata(&self, block_num: u64) -> eyre::Result<Metadata> {
        let block_meta = self
            .client
            .query_one::<BlockInfoData>(BLOCK_INFO, &(block_num))
            .await?
            .value;
        let cex_quotes = self
            .client
            .query_one::<CexPriceData>(CEX_PRICE, &(block_num))
            .await?
            .value;

        let eth_prices = determine_eth_prices(&cex_quotes);

        Ok(BlockMetadata::new(
            block_num,
            block_meta.block_hash,
            block_meta.block_timestamp,
            block_meta.relay_timestamp,
            block_meta.p2p_timestamp,
            block_meta.proposer_fee_recipient,
            block_meta.proposer_mev_reward,
            max(eth_prices.price.0, eth_prices.price.1),
            block_meta.private_flow.into_iter().collect(),
        )
        .into_metadata(cex_quotes, None, None))
    }

    async fn query_many_range<T, D>(&self, start_block: u64, end_block: u64) -> eyre::Result<Vec<D>>
    where
        T: CompressedTable,
        T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
        D: LibmdbxData<T> + DbRow + for<'de> Deserialize<'de> + Send + Debug + 'static,
    {
        self.client
            .query_many::<D>(
                T::INIT_QUERY.expect("no init query found for clickhouse query"),
                &(start_block, end_block),
            )
            .await
            .map_err(Into::into)
    }

    async fn query_many<T, D>(&self) -> eyre::Result<Vec<D>>
    where
        T: CompressedTable,
        T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
        D: LibmdbxData<T> + DbRow + for<'de> Deserialize<'de> + Send + Debug + 'static,
    {
        self.client
            .query_many::<D>(
                T::INIT_QUERY.expect("no init query found for clickhouse query"),
                &(),
            )
            .await
            .map_err(Into::into)
    }

    fn inner(&self) -> &ClickhouseClient<BrontesClickhouseTables> {
        &self.client
    }
}
