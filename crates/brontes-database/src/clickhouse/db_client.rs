use std::{cmp::max, fmt::Debug};

use ::clickhouse::DbRow;
use alloy_primitives::Address;
use brontes_types::{
    db::{
        builder::{BuilderInfo, BuilderStats},
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

use super::{
    dbms::{BrontesClickhouseTables, ClickhouseTxTraces},
    ClickhouseHandle,
};
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
    pub async fn write_searcher_eoa_info(
        &self,
        _searcher_eoa: Address,
        _searcher_info: SearcherInfo,
    ) -> eyre::Result<()> {
        // self.client
        //     .insert_one::<ClickhouseSearcherInfo>(&searcher_info)
        //     .await?;

        Ok(())
    }

    pub async fn write_searcher_contract_info(
        &self,
        _searcher_eoa: Address,
        _searcher_info: SearcherInfo,
    ) -> eyre::Result<()> {
        // self.client
        //     .insert_one::<ClickhouseSearcherInfo>(&searcher_info)
        //     .await?;

        Ok(())
    }

    pub async fn write_searcher_stats(
        &self,
        _searcher_eoa: Address,
        _searcher_stats: SearcherInfo,
    ) -> eyre::Result<()> {
        Ok(())
    }

    pub async fn write_builder_info(
        &self,
        _builder_eoa: Address,
        _builder_info: BuilderInfo,
    ) -> eyre::Result<()> {
        Ok(())
    }

    pub async fn write_builder_stats(
        &self,
        _builder_eoa: Address,
        _builder_stats: BuilderStats,
    ) -> eyre::Result<()> {
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

    pub async fn save_traces(&self, _block: u64, traces: Vec<TxTrace>) -> eyre::Result<()> {
        self.client
            .insert_many::<ClickhouseTxTraces>(&traces)
            .await?;

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

#[cfg(test)]
mod tests {
    use brontes_core::{get_db_handle, init_trace_parser};
    use tokio::sync::mpsc::unbounded_channel;

    use super::*;
    use crate::TxTraces;

    fn spawn_clickhouse() -> Clickhouse {
        dotenv::dotenv().ok();

        Clickhouse::default()
    }

    #[tokio::test]
    async fn tx_traces() {
        let db = spawn_clickhouse();

        let libmdbx = get_db_handle();
        let (a, _b) = unbounded_channel();
        let tracer = init_trace_parser(tokio::runtime::Handle::current(), a, libmdbx, 10).await;

        let binding = tracer.execute_block(18900000).await.unwrap();
        let exec = binding.0.first().unwrap().clone();

        db.inner()
            .insert_one::<ClickhouseTxTraces>(&exec)
            .await
            .unwrap();

        let des: TxTrace = db.inner().query_one(INIT_QUERY, &()).await.unwrap();

        assert_eq!(des, exec);
    }

    const INIT_QUERY: &str = "WITH
    meta AS (
        SELECT 
            tx_hash,
            arrayZip(
                trace_meta.trace_idx,
                trace_meta.msg_sender,
                trace_meta.error,
                trace_meta.subtraces,
                trace_meta.trace_address
            ) AS data
        FROM brontes.tx_traces
    ),
    decoded_data AS (
        SELECT 
            tx_hash,
            arrayZip(
                trace_decoded_data.trace_idx,
                trace_decoded_data.function_name,
                trace_decoded_data.call_data,
                trace_decoded_data.return_data
            ) AS data
        FROM brontes.tx_traces
    ),
    logs AS (
        SELECT 
            tx_hash,
            arrayZip(
                trace_logs.trace_idx,
                trace_logs.log_idx,
                trace_logs.address,
                trace_logs.topics,
                trace_logs.data
            ) AS data
        FROM brontes.tx_traces
    ),
    logs AS (
        SELECT 
            tx_hash,
            arrayZip(
                trace_logs.trace_idx,
                trace_logs.log_idx,
                trace_logs.address,
                trace_logs.topics,
                trace_logs.data
            ) AS data
        FROM brontes.tx_traces
    ),
    actions AS (
        SELECT
            tx_hash,
        arrayZip(
            trace_create_actions.trace_idx,
            trace_create_actions.from,
            trace_create_actions.gas,
            trace_create_actions.init,
            trace_create_actions.value
        ) AS create,
        arrayZip(
            trace_call_actions.trace_idx,
            trace_call_actions.from,
            trace_call_actions.call_type,
            trace_call_actions.gas,
            trace_call_actions.input,
            trace_call_actions.to,
            trace_call_actions.value
        ) AS call,
        arrayZip(
            trace_self_destruct_actions.trace_idx,
            trace_self_destruct_actions.address,
            trace_self_destruct_actions.balance,
            trace_self_destruct_actions.refund_address
        ) AS self_destr,
        arrayZip(
            trace_reward_actions.trace_idx,
            trace_reward_actions.author,
            trace_reward_actions.reward_type,
            trace_reward_actions.value
        ) AS reward
        FROM brontes.tx_traces
    ),
    outputs AS (
        SELECT
            tx_hash,
        arrayZip(
            trace_call_outputs.trace_idx,
            trace_call_outputs.gas_used,
            trace_call_outputs.output
        ) AS call,
        arrayZip(
            trace_create_outputs.trace_idx,
            trace_create_outputs.address,
            trace_create_outputs.code,
            trace_create_outputs.gas_used
        ) AS create
        FROM brontes.tx_traces
    )
SELECT
    (
        m.data, 
        d.data, 
        l.data, 
        a.create,
        a.call,
        a.self_destr,
        a.reward,
        o.call,
        o.create
    ) AS trace,
    tx_traces.tx_hash,
    tx_traces.gas_used,
    tx_traces.effective_price,
    tx_traces.tx_index,
    tx_traces.is_success
FROM brontes.tx_traces AS tx_traces
INNER JOIN meta AS m ON m.tx_hash = tx_traces.tx_hash
INNER JOIN decoded_data AS d ON d.tx_hash = tx_traces.tx_hash
INNER JOIN logs AS l ON l.tx_hash = tx_traces.tx_hash
INNER JOIN actions AS a ON a.tx_hash = tx_traces.tx_hash
INNER JOIN outputs AS o ON o.tx_hash = tx_traces.tx_hash
";
}
