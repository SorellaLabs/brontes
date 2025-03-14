use std::fmt::Debug;
use std::str::FromStr;

use ::clickhouse::DbRow;
use alloy_primitives::Address;
use alloy_primitives::{BlockHash, TxHash};
use async_rate_limiter::{RateLimiter, RateLimiterBuilder, TimeUnit};
use backon::{ExponentialBuilder, Retryable};
#[cfg(feature = "local-clickhouse")]
use brontes_types::db::{block_times::BlockTimes, cex::CexSymbols};
use brontes_types::{
    block_metadata::Relays,
    db::{
        address_to_protocol_info::ProtocolInfoClickhouse,
        block_analysis::BlockAnalysis,
        builder::BuilderInfo,
        cex::{
            quotes::{CexQuotesConverter, RawCexQuotes},
            trades::{CexTradesConverter, RawCexTrades},
            BestCexPerPair,
        },
        dex::{DexQuotes, DexQuotesWithBlockNumber},
        metadata::{BlockMetadata, BlockMetadataInner, Metadata},
        normalized_actions::TransactionRoot,
        searcher::SearcherInfo,
        token_info::{TokenInfo, TokenInfoWithAddress},
    },
    mev::{Bundle, BundleData, MevBlock},
    normalized_actions::Action,
    structured_trace::TxTrace,
    BlockTree, Protocol,
};
use clickhouse::error::Error::{BadResponse, Custom, Network};
use db_interfaces::{
    clickhouse::{
        client::ClickhouseClient, config::ClickhouseConfig, errors::ClickhouseError,
        types::ClickhouseQuery,
    },
    errors::DatabaseError,
    params::BindParameters,
    Database,
};
use eyre::Result;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use tokio::{sync::mpsc::UnboundedSender, time::Duration};
use tracing::{debug, error, warn};

use super::{
    cex_config::CexDownloadConfig, dbms::*, ClickhouseHandle, MOST_VOLUME_PAIR_EXCHANGE,
    P2P_OBSERVATIONS, PRIVATE_FLOW, RAW_CEX_QUOTES, RAW_CEX_TRADES,
};
#[cfg(feature = "local-clickhouse")]
use super::{BLOCK_TIMES, CEX_SYMBOLS};
#[cfg(feature = "local-clickhouse")]
use crate::libmdbx::cex_utils::CexRangeOrArbitrary;
use crate::{
    clickhouse::const_sql::CRIT_INIT_TABLES,
    libmdbx::{determine_eth_prices, tables::CexPriceData, types::LibmdbxData},
    CompressedTable,
};

const SECONDS_TO_US: f64 = 1_000_000.0;
const MAX_MARKOUT_TIME: f64 = 300.0;

#[derive(Clone)]
pub struct Clickhouse {
    pub tip: bool,
    pub run_id: u64,
    pub client: ClickhouseClient<BrontesClickhouseTables>,
    pub rate_limiter: RateLimiter,
    pub cex_download_config: CexDownloadConfig,
    pub buffered_insert_tx: Option<UnboundedSender<Vec<BrontesClickhouseData>>>,
}

impl Clickhouse {
    pub async fn new(
        config: ClickhouseConfig,
        cex_download_config: CexDownloadConfig,
        buffered_insert_tx: Option<UnboundedSender<Vec<BrontesClickhouseData>>>,
        tip: bool,
        run_id: Option<u64>,
    ) -> Self {
        let client = config.build();
        let mut this = Self {
            client,
            cex_download_config,
            buffered_insert_tx,
            tip,
            run_id: 0,
            rate_limiter: RateLimiterBuilder::new(TimeUnit::Minute(1))
                .with_attempts(6)
                .build(),
        };

        this.run_id = if let Some(run_id) = run_id {
            run_id
        } else {
            this.get_and_inc_run_id()
                .await
                .expect("failed to set run_id")
        };
        this
    }

    pub async fn new_default(run_id: Option<u64>) -> Self {
        Clickhouse::new(clickhouse_config(), Default::default(), Default::default(), false, run_id)
            .await
    }

    pub fn inner(&self) -> &ClickhouseClient<BrontesClickhouseTables> {
        &self.client
    }

    pub async fn get_and_inc_run_id(&self) -> eyre::Result<u64> {
        let id = (self
            .client
            .query_one::<u64, _>("select max(run_id) from brontes.run_id", &())
            .await?
            + 1)
        .into();

        self.client.insert_one::<BrontesRun_Id>(&id).await?;

        Ok(id.run_id)
    }

    pub async fn max_traced_block(&self) -> eyre::Result<u64> {
        Ok(self
            .client
            .query_one::<u64, _>("select max(block_number) from brontes_api.tx_traces", &())
            .await?)
    }

    // inserts
    pub async fn write_searcher_eoa_info(
        &self,
        _searcher_eoa: Address,
        _searcher_info: SearcherInfo,
    ) -> eyre::Result<()> {
        Ok(())
    }

    pub async fn write_searcher_contract_info(
        &self,
        _searcher_contract: Address,
        _searcher_info: SearcherInfo,
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

    pub async fn save_mev_blocks(
        &self,
        _block_number: u64,
        block: MevBlock,
        mev: Vec<Bundle>,
    ) -> eyre::Result<()> {
        if let Some(tx) = self.buffered_insert_tx.as_ref() {
            tx.send(vec![(block, self.tip, self.run_id).into()])?;

            let (bundle_headers, bundle_data): (Vec<_>, Vec<_>) = mev
                .into_iter()
                .map(|bundle| (bundle.header, bundle.data))
                .unzip();

            tx.send(
                bundle_headers
                    .into_iter()
                    .map(|a| (a, self.tip, self.run_id))
                    .map(Into::into)
                    .collect(),
            )?;

            bundle_data.into_iter().try_for_each(|data| {
                match data {
                    BundleData::Sandwich(s) => tx.send(vec![(s, self.tip, self.run_id).into()])?,
                    BundleData::AtomicArb(s) => tx.send(vec![(s, self.tip, self.run_id).into()])?,
                    BundleData::JitSandwich(s) => {
                        tx.send(vec![(s, self.tip, self.run_id).into()])?
                    }
                    BundleData::Jit(s) => tx.send(vec![(s, self.tip, self.run_id).into()])?,
                    BundleData::CexDex(s) => tx.send(vec![(s, self.tip, self.run_id).into()])?,
                    BundleData::CexDexQuote(s) => {
                        tx.send(vec![(s, self.tip, self.run_id).into()])?
                    }
                    BundleData::Liquidation(s) => {
                        tx.send(vec![(s, self.tip, self.run_id).into()])?
                    }
                    BundleData::Unknown(s) => tx.send(vec![(s, self.tip, self.run_id).into()])?,
                };

                Ok(()) as eyre::Result<()>
            })?;
        }

        Ok(())
    }

    pub async fn write_dex_quotes(
        &self,
        block_num: u64,
        quotes: Option<DexQuotes>,
    ) -> eyre::Result<()> {
        if let Some(q) = quotes {
            let quotes_with_block = DexQuotesWithBlockNumber::new_with_block(block_num, q);

            if let Some(tx) = self.buffered_insert_tx.as_ref() {
                tx.send(
                    quotes_with_block
                        .into_iter()
                        .zip(vec![self.tip].into_iter().cycle())
                        .map(Into::into)
                        .collect(),
                )?;
            }
        }

        Ok(())
    }

    pub async fn insert_tree(&self, tree: BlockTree<Action>) -> eyre::Result<()> {
        let roots: Vec<TransactionRoot> = tree
            .tx_roots
            .iter()
            .map(|root| (root, tree.header.number).into())
            .collect::<Vec<_>>();

        if let Some(tx) = self.buffered_insert_tx.as_ref() {
            tx.send(
                roots
                    .into_iter()
                    .map(|root| (root, self.tip, self.run_id))
                    .map(Into::into)
                    .collect(),
            )?;
        }

        Ok(())
    }

    pub async fn write_token_info(
        &self,
        address: Address,
        decimals: u8,
        symbol: String,
    ) -> eyre::Result<()> {
        let data = TokenInfoWithAddress { address, inner: TokenInfo { symbol, decimals } };

        if let Some(tx) = self.buffered_insert_tx.as_ref() {
            tx.send(vec![(data, self.tip).into()])?
        };

        Ok(())
    }

    pub async fn insert_pool(
        &self,
        block: u64,
        address: Address,
        tokens: &[Address],
        curve_lp_token: Option<Address>,
        classifier_name: Protocol,
    ) -> eyre::Result<()> {
        let data =
            ProtocolInfoClickhouse::new(block, address, tokens, curve_lp_token, classifier_name);

        if let Some(tx) = self.buffered_insert_tx.as_ref() {
            tx.send(vec![(data, self.tip).into()])?
        };

        Ok(())
    }

    pub async fn block_analysis(&self, block_analysis: BlockAnalysis) -> eyre::Result<()> {
        if let Some(tx) = self.buffered_insert_tx.as_ref() {
            tx.send(vec![(block_analysis, self.tip, self.run_id).into()])?
        };

        Ok(())
    }

    pub async fn save_traces(&self, _block: u64, _traces: Vec<TxTrace>) -> eyre::Result<()> {
        Ok(())
    }

    async fn query_many_with_retry<Q, P>(
        &self,
        query: impl AsRef<str> + Send,
        params: &P,
    ) -> Result<Vec<Q>, DatabaseError>
    where
        Q: ClickhouseQuery,
        P: BindParameters + Send + Sync,
    {
        let retry_strategy = ExponentialBuilder::default()
            .with_max_times(10)
            .with_min_delay(Duration::from_millis(100))
            .with_max_delay(Duration::from_secs(30));

        let mut try_count = 1;
        let res = (|| async { self.client.query_many::<Q, P>(query.as_ref(), params).await })
            .retry(&retry_strategy)
            .when(|e| match e {
                DatabaseError::ClickhouseError(ClickhouseError::ClickhouseNative(Network(_))) => {
                    true
                }
                DatabaseError::ClickhouseError(ClickhouseError::ClickhouseNative(BadResponse(
                    s,
                ))) if s.to_string().contains("MEMORY_LIMIT_EXCEEDED") => true,
                _ => false,
            })
            .notify(|err, dur| {
                warn!(
                    "Query failed after {} attempt(s).  Retrying in {:?}... Error: {}",
                    try_count, dur, err
                );
                try_count += 1;
            })
            .await;
        match res {
            Ok(result) => Ok(result),
            Err(err) => {
                error!("Query failed after maximum retries - final Error: {}", err);
                Err(DatabaseError::ClickhouseError(ClickhouseError::ClickhouseNative(Custom(
                    "Query failed after maximum retries".to_string(),
                ))))
            }
        }
    }

    async fn query_optional_with_retry<Q, P>(
        &self,
        query: impl AsRef<str> + Send,
        params: &P,
    ) -> Result<Option<Q>, DatabaseError>
    where
        Q: ClickhouseQuery,
        P: BindParameters + Send + Sync,
    {
        let retry_strategy = ExponentialBuilder::default()
            .with_max_times(10)
            .with_min_delay(Duration::from_millis(100))
            .with_max_delay(Duration::from_secs(30));

        let mut try_count = 1;
        let res = (|| async {
            self.client
                .query_one_optional::<Q, P>(query.as_ref(), params)
                .await
        })
        .retry(&retry_strategy)
        .when(|e| match e {
            DatabaseError::ClickhouseError(ClickhouseError::ClickhouseNative(Network(_))) => true,
            DatabaseError::ClickhouseError(ClickhouseError::ClickhouseNative(BadResponse(s)))
                if s.to_string().contains("MEMORY_LIMIT_EXCEEDED") =>
            {
                true
            }
            _ => false,
        })
        .notify(|err, dur| {
            warn!(
                "Query failed after {} attempt(s).  Retrying in {:?}... Error: {}",
                try_count, dur, err
            );
            try_count += 1;
        })
        .await;
        match res {
            Ok(result) => Ok(result),
            Err(err) => {
                error!("Query failed after maximum retries - final Error: {}", err);
                Err(DatabaseError::ClickhouseError(ClickhouseError::ClickhouseNative(Custom(
                    "Query failed after maximum retries".to_string(),
                ))))
            }
        }
    }

    pub async fn get_private_flow(
        &self,
        mut tx_hashes_in_block: Vec<TxHash>,
    ) -> eyre::Result<Vec<TxHash>> {
        if tx_hashes_in_block.is_empty() {
            return Ok(Vec::new());
        }

        let public_txs = self
            .rate_limiter
            .run_with(
                self.query_many_with_retry(
                    PRIVATE_FLOW,
                    &(tx_hashes_in_block
                        .iter()
                        .map(|tx| format!("{:?}", tx).to_lowercase())
                        .collect::<Vec<_>>()),
                ),
            )
            .await?
            .into_iter()
            .map(|tx: String| TxHash::from_str(&tx))
            .collect::<Result<Vec<_>, _>>()?;

        if !public_txs.is_empty() {
            tx_hashes_in_block.retain(|tx| !public_txs.contains(tx));
            Ok(tx_hashes_in_block)
        } else {
            Ok(Vec::new())
        }
    }

    pub async fn get_earliest_p2p_observation(
        &self,
        block_number: u64,
        block_hash: BlockHash,
    ) -> eyre::Result<Option<u64>> {
        Ok(self
            .rate_limiter
            .run_with(self.query_optional_with_retry::<u64, _>(
                P2P_OBSERVATIONS,
                &(block_number, format!("{:?}", block_hash).to_lowercase()),
            ))
            .await?)
    }
}

impl ClickhouseHandle for Clickhouse {
    async fn get_init_crit_tables(&self) -> eyre::Result<ClickhouseCritTableCount> {
        let res: ClickhouseCritTableCount = self.client.query_one(CRIT_INIT_TABLES, &()).await?;
        Ok(res)
    }

    async fn get_metadata(
        &self,
        block_num: u64,
        block_timestamp: u64,
        block_hash: BlockHash,
        tx_hashes_in_block: Vec<TxHash>,
        quote_asset: Address,
    ) -> eyre::Result<Metadata> {
        let (relay, p2p_timestamp, private_flow) = tokio::try_join!(
            Relays::get_relay_metadata(block_num, block_hash),
            self.get_earliest_p2p_observation(block_num, block_hash),
            self.get_private_flow(tx_hashes_in_block)
        )
        .inspect_err(|e| {
            tracing::warn!("error getting block metadata - {:?}", e);
        })?;

        let block_meta = BlockMetadataInner::make_new(
            block_hash,
            block_timestamp,
            relay,
            p2p_timestamp,
            private_flow,
        );

        let mut cex_quotes_for_block = self
            .get_cex_prices(CexRangeOrArbitrary::Timestamp {
                block_number: block_num,
                block_timestamp,
            })
            .await?;

        if cex_quotes_for_block.is_empty() {
            tracing::warn!("loaded zero cex quotes. check backend");
            return Err(eyre::eyre!("error loading cex quotes"));
        }

        let cex_quotes = cex_quotes_for_block.remove(0);
        let eth_price = determine_eth_prices(
            &cex_quotes.value,
            block_meta.block_timestamp * 1_000_000,
            quote_asset,
        );

        let meta = BlockMetadata::new(
            block_num,
            block_meta.block_hash,
            block_meta.block_timestamp,
            block_meta.relay_timestamp,
            block_meta.p2p_timestamp,
            block_meta.proposer_fee_recipient,
            block_meta.proposer_mev_reward,
            eth_price.unwrap_or_default(),
            block_meta.private_flow.into_iter().collect(),
        )
        .into_metadata(cex_quotes.value, None, None, None);

        Ok(meta)
    }

    async fn query_many_range<T, D>(&self, start_block: u64, end_block: u64) -> eyre::Result<Vec<D>>
    where
        T: CompressedTable,
        T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
        D: LibmdbxData<T> + DbRow + for<'de> Deserialize<'de> + Send + Debug + 'static,
    {
        self.query_many_with_retry::<D, _>(
            T::INIT_QUERY.expect("no init query found for clickhouse query"),
            &(start_block, end_block),
        )
        .await
        .map_err(Into::into)
    }

    async fn query_many_arbitrary<T, D>(&self, range: &'static [u64]) -> eyre::Result<Vec<D>>
    where
        T: CompressedTable,
        T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
        D: LibmdbxData<T> + DbRow + for<'de> Deserialize<'de> + Send + Debug + 'static,
    {
        let query = format_arbitrary_query::<T>(range);

        self.query_many_with_retry::<D, _>(&query, &())
            .await
            .map_err(Into::into)
    }

    async fn query_many<T, D>(&self) -> eyre::Result<Vec<D>>
    where
        T: CompressedTable,
        T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
        D: LibmdbxData<T> + DbRow + for<'de> Deserialize<'de> + Send + Debug + 'static,
    {
        self.query_many_with_retry::<D, _>(
            T::INIT_QUERY.expect("no init query found for clickhouse query"),
            &(),
        )
        .await
        .map_err(Into::into)
    }

    fn inner(&self) -> &ClickhouseClient<BrontesClickhouseTables> {
        &self.client
    }

    async fn get_cex_prices(
        &self,
        range_or_arbitrary: CexRangeOrArbitrary,
    ) -> eyre::Result<Vec<crate::CexPriceData>> {
        let block_times: Vec<BlockTimes> = match range_or_arbitrary {
            CexRangeOrArbitrary::Range(s, e) => {
                debug!(
                    target = "b",
                    "Querying block times to download quotes for range: start={}, end={}", s, e
                );
                self.query_many_with_retry(BLOCK_TIMES, &(s, e)).await?
            }

            CexRangeOrArbitrary::Arbitrary(vals) => {
                let mut query = BLOCK_TIMES.to_string();

                let vals = vals
                    .iter()
                    .flat_map(|v| {
                        (v - self.cex_download_config.run_time_window.0
                            ..=v + self.cex_download_config.run_time_window.1)
                            .collect_vec()
                    })
                    .unique()
                    .collect::<Vec<_>>();

                query = query.replace(
                    "block_number >= ? AND block_number < ?",
                    &format!("block_number IN (SELECT arrayJoin({:?}) AS block_number)", vals),
                );

                self.client.query_many(query, &()).await?
            }
            CexRangeOrArbitrary::Timestamp { block_number, block_timestamp } => {
                vec![BlockTimes { block_number, timestamp: block_timestamp * 1000000 }]
            }
        };

        if block_times.is_empty() {
            eyre::bail!("No block times found");
        }

        let symbols: Vec<CexSymbols> = self.client.query_many(CEX_SYMBOLS, &()).await?;

        let exchanges_str = self
            .cex_download_config
            .clone()
            .exchanges_to_use
            .into_iter()
            .map(|s| s.to_clickhouse_filter().to_string())
            .collect::<Vec<_>>()
            .join(" OR ");

        tracing::trace!("Fetching symbol ranks");
        let symbol_rank = self
            .fetch_symbol_rank(&block_times, &range_or_arbitrary)
            .await?;

        tracing::trace!("Successfully fetched symbol ranks");

        let data: Vec<RawCexQuotes> = match range_or_arbitrary {
            CexRangeOrArbitrary::Range(..) => {
                let start_time = block_times
                    .iter()
                    .min_by_key(|b| b.timestamp)
                    .map(|b| b.timestamp)
                    .unwrap() as f64
                    - (MAX_MARKOUT_TIME * SECONDS_TO_US);
                let end_time = block_times
                    .iter()
                    .max_by_key(|b| b.timestamp)
                    .map(|b| b.timestamp)
                    .unwrap() as f64
                    + (MAX_MARKOUT_TIME * SECONDS_TO_US);

                let mut query = RAW_CEX_QUOTES.to_string();
                query = query.replace(
                    "c.timestamp >= ? AND c.timestamp < ?",
                    &format!(
                        "c.timestamp >= {} AND c.timestamp < {} AND ({})",
                        start_time, end_time, exchanges_str
                    ),
                );

                self.query_many_with_retry(query, &()).await?
            }
            CexRangeOrArbitrary::Arbitrary(_) => {
                let mut query = RAW_CEX_QUOTES.to_string();

                let query_mod = block_times
                    .iter()
                    .map(|b| {
                        b.convert_to_timestamp_query(
                            MAX_MARKOUT_TIME * SECONDS_TO_US,
                            MAX_MARKOUT_TIME * SECONDS_TO_US,
                        )
                    })
                    .collect::<Vec<String>>()
                    .join(" OR ");

                query = query.replace(
                    "c.timestamp >= ? AND c.timestamp < ?",
                    &format!("({query_mod}) AND ({exchanges_str})"),
                );

                self.query_many_with_retry(query, &()).await?
            }
            CexRangeOrArbitrary::Timestamp { block_number: _, block_timestamp } => {
                let start_time =
                    (block_timestamp * 1000000) as f64 - (MAX_MARKOUT_TIME * SECONDS_TO_US);
                let end_time =
                    (block_timestamp * 1000000) as f64 + (MAX_MARKOUT_TIME * SECONDS_TO_US);

                let mut query = RAW_CEX_QUOTES.to_string();
                query = query.replace(
                    "c.timestamp >= ? AND c.timestamp < ?",
                    &format!(
                        "c.timestamp >= {} AND c.timestamp < {} AND ({})",
                        start_time, end_time, exchanges_str
                    ),
                );

                self.query_many_with_retry(query, &()).await?
            }
        };

        let price_converter = CexQuotesConverter::new(block_times, symbols, data, symbol_rank);
        let prices: Vec<CexPriceData> = price_converter
            .convert_to_prices()
            .into_iter()
            .map(|(block_num, price_map)| CexPriceData::new(block_num, price_map))
            .collect();

        Ok(prices)
    }

    async fn get_cex_trades(
        &self,
        range_or_arbitrary: CexRangeOrArbitrary,
    ) -> eyre::Result<Vec<crate::CexTradesData>> {
        debug!("Starting get_cex_trades function");
        let block_times: Vec<BlockTimes> = match range_or_arbitrary {
            CexRangeOrArbitrary::Range(mut s, mut e) => {
                s -= self.cex_download_config.run_time_window.0;
                e += self.cex_download_config.run_time_window.1;

                debug!(
                    target = "brontes_db::cex_download",
                    "Querying block times to download trades for range: start={}, end={}", s, e
                );
                self.client.query_many(BLOCK_TIMES, &(s, e)).await?
            }
            CexRangeOrArbitrary::Arbitrary(vals) => {
                let vals = vals
                    .iter()
                    .flat_map(|v| {
                        (v - self.cex_download_config.run_time_window.0
                            ..=v + self.cex_download_config.run_time_window.1)
                            .collect_vec()
                    })
                    .unique()
                    .collect::<Vec<_>>();

                debug!("Querying block times for arbitrary values: {:?}", vals);
                let mut query = BLOCK_TIMES.to_string();
                query = query.replace(
                    "block_number >= ? AND block_number < ?",
                    &format!("block_number IN (SELECT arrayJoin({:?}) AS block_number)", vals),
                );
                self.client.query_many(query, &()).await?
            }
            CexRangeOrArbitrary::Timestamp { block_number, block_timestamp } => {
                vec![BlockTimes { block_number, timestamp: block_timestamp * 1000000 }]
            }
        };

        debug!("Retrieved {} block times", block_times.len());

        if block_times.is_empty() {
            eyre::bail!("No block times found");
        }

        debug!("Querying CEX symbols");
        let symbols: Vec<CexSymbols> = self.client.query_many(CEX_SYMBOLS, &()).await?;
        debug!("Retrieved {} CEX symbols", symbols.len());

        let exchanges_str = self
            .cex_download_config
            .clone()
            .exchanges_to_use
            .into_iter()
            .map(|s| s.to_clickhouse_filter().to_string())
            .collect::<Vec<String>>()
            .join(" OR ");
        debug!("Using exchanges filter: {}", exchanges_str);

        let data: Vec<RawCexTrades> = match range_or_arbitrary {
            CexRangeOrArbitrary::Range(..) => {
                let start_time = block_times
                    .iter()
                    .min_by_key(|b| b.timestamp)
                    .map(|b| b.timestamp)
                    .unwrap() as f64
                    - (6.0 * SECONDS_TO_US);
                let end_time = block_times
                    .iter()
                    .max_by_key(|b| b.timestamp)
                    .map(|b| b.timestamp)
                    .unwrap() as f64
                    + (6.0 * SECONDS_TO_US);

                debug!(
                    "Querying raw CEX trades for time range: start={}, end={}",
                    start_time, end_time
                );

                let mut query = RAW_CEX_TRADES.to_string();
                query = query.replace(
                    "c.timestamp >= ? AND c.timestamp < ?",
                    &format!(
                        "c.timestamp >= {start_time} AND c.timestamp < {end_time} 
                        and ({exchanges_str})"
                    ),
                );
                self.query_many_with_retry(query, &()).await?
            }
            CexRangeOrArbitrary::Arbitrary(_) => {
                let mut query = RAW_CEX_TRADES.to_string();
                let query_mod = block_times
                    .iter()
                    .map(|b| b.convert_to_timestamp_query(6.0 * SECONDS_TO_US, 6.0 * SECONDS_TO_US))
                    .collect::<Vec<String>>()
                    .join(" OR ");

                debug!("Querying raw CEX trades for arbitrary block times");

                query = query.replace(
                    "c.timestamp >= ? AND c.timestamp < ?",
                    &format!("({query_mod}) AND ({exchanges_str})"),
                );
                self.query_many_with_retry(query, &()).await?
            }

            CexRangeOrArbitrary::Timestamp { block_number, block_timestamp } => {
                let mut query = RAW_CEX_TRADES.to_string();
                let query_mod = BlockTimes { block_number, timestamp: block_timestamp * 1000000 }
                    .convert_to_timestamp_query(6.0 * SECONDS_TO_US, 6.0 * SECONDS_TO_US);

                debug!("Querying raw CEX trades for block number {block_number}");

                query = query.replace(
                    "c.timestamp >= ? AND c.timestamp < ?",
                    &format!("({query_mod}) AND ({exchanges_str})"),
                );

                self.query_many_with_retry(query, &()).await?
            }
        };

        debug!("Retrieved {} raw CEX trades", data.len());

        let trades_converter = CexTradesConverter::new(block_times, symbols, data);

        debug!("Converting raw trades to CexTradesData");
        let trades: Vec<crate::CexTradesData> = trades_converter
            .convert_to_trades()
            .into_iter()
            .map(|(block_num, trade_map)| crate::CexTradesData::new(block_num, trade_map))
            .collect();

        debug!("Converted {} CexTradesData entries", trades.len());

        Ok(trades)
    }
}

impl Clickhouse {
    pub async fn fetch_symbol_rank(
        &self,
        block_times: &[BlockTimes],
        range_or_arbitrary: &CexRangeOrArbitrary,
    ) -> eyre::Result<Vec<BestCexPerPair>, DatabaseError> {
        if block_times.is_empty() {
            return Err(DatabaseError::from(clickhouse::error::Error::Custom(
                "Nothing to query, block times are empty".to_string(),
            )));
        }
        Ok(match range_or_arbitrary {
            CexRangeOrArbitrary::Range(..) => {
                let start_time = block_times
                    .iter()
                    .min_by_key(|b| b.timestamp)
                    .map(|b| b.timestamp)
                    .unwrap() as f64;
                let end_time = block_times
                    .iter()
                    .max_by_key(|b| b.timestamp)
                    .map(|b| b.timestamp)
                    .unwrap() as f64;

                self.query_many_with_retry(MOST_VOLUME_PAIR_EXCHANGE, &(start_time, end_time))
                    .await?
            }
            CexRangeOrArbitrary::Arbitrary(_) => {
                let mut query = MOST_VOLUME_PAIR_EXCHANGE.to_string();

                let times = block_times
                    .iter()
                    .map(|block| {
                        format!(
                            "toStartOfMonth(toDateTime({} /  1000000) - INTERVAL 1 MONTH)",
                            block.timestamp as f64
                        )
                    })
                    .fold(String::new(), |mut acc, x| {
                        if !acc.is_empty() {
                            acc += ",";
                        }

                        acc += &x;
                        acc
                    });

                query = query.replace(
                    "month >= toStartOfMonth(toDateTime(? / 1000000) - toIntervalMonth(1))) AND \
                     (month <= toStartOfMonth(toDateTime(? / 1000000) + toIntervalMonth(1))",
                    &format!("month in (select arrayJoin([{}]) as month)", times),
                );

                self.query_many_with_retry(query, &()).await?
            }
            CexRangeOrArbitrary::Timestamp { block_number: _, block_timestamp } => {
                self.query_many_with_retry(
                    MOST_VOLUME_PAIR_EXCHANGE,
                    &(block_timestamp * 1000000, block_timestamp * 1000000),
                )
                .await?
            }
        })
    }

    pub async fn get_block_times_range(
        &self,
        range: &CexRangeOrArbitrary,
    ) -> Result<Vec<BlockTimes>, db_interfaces::errors::DatabaseError> {
        let (start, end) = match range {
            CexRangeOrArbitrary::Range(start, end) => (start, end),
            CexRangeOrArbitrary::Arbitrary(_) => panic!("Arbitrary range not supported"),
            CexRangeOrArbitrary::Timestamp { .. } => {
                panic!("timestamp based block times not supported")
            }
        };

        debug!(target = "b", "Querying block times for range: start={}, end={}", start, end);
        self.query_many_with_retry(BLOCK_TIMES, &(start, end)).await
    }

    pub async fn get_cex_symbols(
        &self,
    ) -> Result<Vec<CexSymbols>, db_interfaces::errors::DatabaseError> {
        self.query_many_with_retry(CEX_SYMBOLS, &()).await
    }

    pub async fn get_raw_cex_quotes_range(
        &self,
        start_time: u64,
        end_time: u64,
    ) -> Result<Vec<RawCexQuotes>, db_interfaces::errors::DatabaseError> {
        let exchanges_str = self.get_exchanges_filter_string();

        let query = self.build_range_cex_quotes_query(start_time, end_time, &exchanges_str);
        self.query_many_with_retry(&query, &()).await
    }

    fn get_exchanges_filter_string(&self) -> String {
        self.cex_download_config
            .exchanges_to_use
            .iter()
            .map(|s| s.to_clickhouse_filter().to_string())
            .collect::<Vec<_>>()
            .join(" OR ")
    }

    fn build_range_cex_quotes_query(
        &self,
        start_time: u64,
        end_time: u64,
        exchanges_str: &str,
    ) -> String {
        let query = RAW_CEX_QUOTES.to_string();
        query.replace(
            "c.timestamp >= ? AND c.timestamp < ?",
            &format!(
                "c.timestamp >= {} AND c.timestamp < {} AND ({})",
                start_time, end_time, exchanges_str
            ),
        )
    }
}

pub fn clickhouse_config() -> db_interfaces::clickhouse::config::ClickhouseConfig {
    let url = format!(
        "{}:{}",
        std::env::var("CLICKHOUSE_URL").expect("CLICKHOUSE_URL not found in .env"),
        std::env::var("CLICKHOUSE_PORT").expect("CLICKHOUSE_PORT not found in .env")
    );
    let user = std::env::var("CLICKHOUSE_USER").expect("CLICKHOUSE_USER not found in .env");
    let pass = std::env::var("CLICKHOUSE_PASS").expect("CLICKHOUSE_PASS not found in .env");

    db_interfaces::clickhouse::config::ClickhouseConfig::new(user, pass, url, true, None)
}

fn format_arbitrary_query<T>(range: &'static [u64]) -> String
where
    T: CompressedTable,
    T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
{
    let mut query = T::INIT_QUERY
        .expect("no init query found for clickhouse query")
        .to_string();

    query = query.replace(
        "block_number >= ? AND block_number < ?",
        &format!("block_number IN (SELECT arrayJoin({:?}) AS block_number)", range),
    );

    query = query.replace(
        "    ? AS start_block,
    ? AS end_block",
        &format!(
            "    block_numbers AS (
        SELECT
            arrayJoin({:?}) AS block_number
    )",
            range
        ),
    );

    query = query.replace(
        "block_number >= start_block AND block_number < end_block",
        "block_number in block_numbers",
    );

    query
}

#[derive(Debug, Serialize, Deserialize, clickhouse::Row)]
pub struct ClickhouseCritTableCount {
    pub pool_creation: u64,
    pub address_to_protocol: u64,
    pub tokens: u64,
    pub builder: u64,
    pub address_meta: u64,
}

impl ClickhouseCritTableCount {
    pub fn all_greater(&self, clickhouse: ClickhouseCritTableCount) -> bool {
        self.pool_creation >= clickhouse.pool_creation
            && self.address_to_protocol >= clickhouse.address_to_protocol
            && self.tokens >= clickhouse.tokens
            && self.builder >= clickhouse.builder
            && self.address_meta >= clickhouse.address_meta
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use alloy_primitives::{hex, Uint};
    use brontes_classifier::test_utils::ClassifierTestUtils;
    use brontes_types::{
        block_metadata::RelayBlockMetadata,
        db::{cex::CexExchange, dex::DexPrices, DbDataWithRunId},
        init_thread_pools,
        mev::{
            ArbDetails, AtomicArb, BundleHeader, CexDex, CexDexQuote, JitLiquidity,
            JitLiquiditySandwich, Liquidation, OptimisticTrade, PossibleMev, PossibleMevCollection,
            Sandwich,
        },
        normalized_actions::{
            NormalizedBurn, NormalizedLiquidation, NormalizedMint, NormalizedSwap,
        },
        pair::Pair,
        FastHashMap, GasDetails,
    };
    use db_interfaces::{
        clickhouse::{dbms::ClickhouseDBMS, test_utils::ClickhouseTestClient},
        test_utils::TestDatabase,
    };
    use malachite::{num::basic::traits::Zero, Rational};

    use super::*;

    #[brontes_macros::test]
    async fn test_get_private_flow() {
        let tx_hashes = [
            "0x0000000000051fd2c9e99dcb6037d17950a377f21e10ae13f7e3ff0487b74e97",
            "0x00000000000aac1650b3ebdc98ff4cf0f5c295a37fa04eddde156d75cac48222",
        ]
        .iter()
        .map(|t| TxHash::from_str(t).unwrap())
        .collect();

        let test_db = Clickhouse::new_default(Some(0)).await;
        let private_flow = test_db.get_private_flow(tx_hashes).await.unwrap();

        assert_eq!(
            private_flow,
            vec![TxHash::from_str(
                "0x0000000000051fd2c9e99dcb6037d17950a377f21e10ae13f7e3ff0487b74e97"
            )
            .unwrap()]
        );
    }

    #[brontes_macros::test]
    async fn test_get_earliest_p2p_observation() {
        let block_number = 19000000;
        let block_hash = BlockHash::from_str(
            "0xcf384012b91b081230cdf17a3f7dd370d8e67056058af6b272b3d54aa2714fac",
        )
        .unwrap();

        let test_db = Clickhouse::new_default(Some(0)).await;
        let earliest_p2p = test_db
            .get_earliest_p2p_observation(block_number, block_hash)
            .await
            .unwrap();

        assert_eq!(earliest_p2p, Some(1705173444789));
    }

    #[brontes_macros::test]
    async fn test_get_relay_metadata() {
        let block_number = 19000000;
        let block_hash = BlockHash::from_str(
            "0xcf384012b91b081230cdf17a3f7dd370d8e67056058af6b272b3d54aa2714fac",
        )
        .unwrap();

        let relay_meta = Relays::get_relay_metadata(block_number, block_hash)
            .await
            .unwrap();

        if relay_meta.is_some() {
            assert_eq!(
                relay_meta,
                Some(RelayBlockMetadata {
                    block_number,
                    relay_timestamp: Some(1705173443953),
                    proposer_fee_recipient: Address::from_str(
                        "0x992a7a7d9267d114959dd0c9d072d965c4f54419"
                    )
                    .unwrap(),
                    proposer_mev_reward: 83855601164275442
                })
            );
        }
    }

    #[brontes_macros::test]
    async fn test_get_cex_quotes_timestamp() {
        init_thread_pools(32);
        let test_db = Clickhouse::new_default(Some(0)).await;
        let cex_quotes_for_block = test_db
            .get_cex_prices(CexRangeOrArbitrary::Timestamp {
                block_number: 19000000,
                block_timestamp: 1705173443,
            })
            .await
            .unwrap();

        assert!(!cex_quotes_for_block.is_empty());
    }

    #[brontes_macros::test]
    async fn test_get_cex_trades_timestamp() {
        init_thread_pools(32);
        let test_db = Clickhouse::new_default(Some(0)).await;
        let cex_trades_for_block = test_db
            .get_cex_trades(CexRangeOrArbitrary::Timestamp {
                block_number: 19000000,
                block_timestamp: 1705173443,
            })
            .await
            .unwrap();

        assert!(!cex_trades_for_block.is_empty());
    }

    async fn load_tree() -> Arc<BlockTree<Action>> {
        let classifier_utils = ClassifierTestUtils::new().await;
        let tx = hex!("31dedbae6a8e44ec25f660b3cd0e04524c6476a0431ab610bb4096f82271831b").into();
        classifier_utils.build_tree_tx(tx).await.unwrap().into()
    }

    async fn token_info(db: &ClickhouseTestClient<BrontesClickhouseTables>) {
        let case0 = TokenInfoWithAddress::default();

        db.insert_one::<BrontesToken_Info>(&case0).await.unwrap();
    }

    async fn dex_price_mapping(db: &ClickhouseTestClient<BrontesClickhouseTables>) {
        let case0_pair = Pair::default();
        let case0_dex_prices = DexPrices::default();
        let mut case0_map = FastHashMap::default();
        case0_map.insert(case0_pair, case0_dex_prices);

        let case0 = DexQuotesWithBlockNumber {
            block_number: Default::default(),
            tx_idx: Default::default(),
            quote: Some(case0_map),
        };

        db.insert_one::<BrontesDex_Price_Mapping>(&case0)
            .await
            .unwrap();

        let query = "SELECT * FROM brontes.dex_price_mapping";
        let queried: DexQuotesWithBlockNumber = db.query_one(query, &()).await.unwrap();

        assert_eq!(queried, case0);
    }

    async fn mev_block(db: &ClickhouseTestClient<BrontesClickhouseTables>) {
        let case0_possible = PossibleMev::default();
        let case0 = MevBlock {
            possible_mev: PossibleMevCollection(vec![case0_possible]),
            ..Default::default()
        };

        db.insert_one::<MevMev_Blocks>(&DbDataWithRunId::new_with_run_id(case0, 0))
            .await
            .unwrap();
    }

    async fn cex_dex(db: &ClickhouseTestClient<BrontesClickhouseTables>) {
        let swap = NormalizedSwap::default();
        let arb_detail = ArbDetails::default();
        let opt_trade = OptimisticTrade::default();
        let cex_exchange = CexExchange::Binance;

        let case0 = CexDex {
            swaps: vec![swap.clone()],
            global_vmap_details: vec![arb_detail.clone()],
            optimal_route_details: vec![arb_detail.clone()],
            optimistic_route_details: vec![arb_detail.clone()],
            optimistic_trade_details: vec![vec![opt_trade.clone()]],
            per_exchange_details: vec![vec![arb_detail.clone()]],
            per_exchange_pnl: vec![(cex_exchange, (Rational::ZERO, Rational::ZERO))],
            ..CexDex::default()
        };

        db.insert_one::<MevCex_Dex>(&DbDataWithRunId::new_with_run_id(case0, 0))
            .await
            .unwrap();

        let case1 = CexDex {
            swaps: vec![swap.clone()],
            global_vmap_details: vec![arb_detail.clone()],
            optimal_route_details: vec![arb_detail.clone()],
            optimistic_route_details: vec![arb_detail.clone()],
            optimistic_trade_details: vec![vec![opt_trade.clone()]],
            per_exchange_details: vec![vec![arb_detail.clone()]],
            per_exchange_pnl: vec![(cex_exchange, (Rational::ZERO, Rational::ZERO))],
            ..CexDex::default()
        };

        db.insert_one::<MevCex_Dex>(&DbDataWithRunId::new_with_run_id(case1, 0))
            .await
            .unwrap();
    }

    async fn cex_dex_quotes(db: &ClickhouseTestClient<BrontesClickhouseTables>) {
        let swap = NormalizedSwap {
            protocol: Protocol::UniswapV2,
            from: hex!("a69babef1ca67a37ffaf7a485dfff3382056e78c").into(),
            recipient: hex!("a69babef1ca67a37ffaf7a485dfff3382056e78c").into(),
            pool: hex!("88e6a0c2ddd26feeb64f039a2c41296fcb3f5640").into(),
            token_in: TokenInfoWithAddress::weth(),
            token_out: TokenInfoWithAddress::usdc(),
            amount_in: Rational::from_unsigneds(
                3122757495341445439573u128,
                1000000000000000000u128,
            ),
            amount_out: Rational::from_unsigneds(1254253571443u64, 250000u64),
            trace_index: 2,
            msg_value: Uint::from(0),
        };

        let case0 = CexDexQuote {
            tx_hash: hex!("ba217d10561a1cd6c52830dcc673886901e69ddb4db5e50c83f39ff0cfd14377")
                .into(),
            block_timestamp: 1694364587,
            block_number: 18107273,
            swaps: vec![swap],
            instant_mid_price: vec![0.0006263290093187073],
            t2_mid_price: vec![0.0006263290093187073],
            t12_mid_price: vec![0.0006263290093187073],
            t30_mid_price: vec![0.0006263290093187073],
            t60_mid_price: vec![0.0006263290093187073],
            t300_mid_price: vec![0.0006263290093187073],
            exchange: CexExchange::Binance,
            pnl: 12951.829205242997,
            gas_details: GasDetails {
                coinbase_transfer: Some(11419369165096275986),
                priority_fee: 0,
                gas_used: 271686,
                effective_gas_price: 8875282233,
            },
        };

        db.insert_one::<MevCex_Dex_Quotes>(&DbDataWithRunId::new_with_run_id(case0, 42069))
            .await
            .unwrap();
    }

    async fn jit(db: &ClickhouseTestClient<BrontesClickhouseTables>) {
        let swap = NormalizedSwap::default();
        let mint = NormalizedMint::default();
        let burn = NormalizedBurn::default();
        let gas_details = GasDetails::default();
        let case0 = JitLiquidity {
            frontrun_mints: vec![mint],
            backrun_burns: vec![burn],
            victim_swaps: vec![vec![swap]],
            victim_swaps_gas_details: vec![gas_details],
            ..JitLiquidity::default()
        };

        db.insert_one::<MevJit>(&DbDataWithRunId::new_with_run_id(case0, 0))
            .await
            .unwrap();
    }

    async fn jit_sandwich(db: &ClickhouseTestClient<BrontesClickhouseTables>) {
        let swap = NormalizedSwap::default();
        let mint = NormalizedMint::default();
        let burn = NormalizedBurn::default();
        let gas_details = GasDetails::default();
        let case0 = JitLiquiditySandwich {
            frontrun_mints: vec![Some(vec![mint])],
            backrun_burns: vec![burn],
            victim_swaps: vec![vec![swap]],
            victim_swaps_gas_details: vec![gas_details],
            ..JitLiquiditySandwich::default()
        };

        db.insert_one::<MevJit_Sandwich>(&DbDataWithRunId::new_with_run_id(case0, 0))
            .await
            .unwrap();
    }

    async fn liquidations(db: &ClickhouseTestClient<BrontesClickhouseTables>) {
        let swap = NormalizedSwap::default();
        let liquidation = NormalizedLiquidation::default();
        let gas_details = GasDetails::default();
        let case0 = Liquidation {
            liquidation_swaps: vec![swap],
            liquidations: vec![liquidation],
            gas_details,
            ..Liquidation::default()
        };

        db.insert_one::<MevLiquidations>(&DbDataWithRunId::new_with_run_id(case0, 0))
            .await
            .unwrap();
    }

    async fn bundle_header(db: &ClickhouseTestClient<BrontesClickhouseTables>) {
        let case0 = BundleHeader::default();

        db.insert_one::<MevBundle_Header>(&DbDataWithRunId::new_with_run_id(case0, 0))
            .await
            .unwrap();
    }

    async fn sandwich(db: &ClickhouseTestClient<BrontesClickhouseTables>) {
        let swap0 = NormalizedSwap::default();
        let swap1 = NormalizedSwap::default();
        let swap2 = NormalizedSwap::default();
        let gas_details = GasDetails::default();
        let case0 = Sandwich {
            frontrun_swaps: vec![vec![swap0]],
            victim_swaps: vec![vec![swap1]],
            victim_swaps_gas_details: vec![gas_details],
            backrun_swaps: vec![swap2],
            ..Sandwich::default()
        };

        db.insert_one::<MevSandwiches>(&DbDataWithRunId::new_with_run_id(case0, 0))
            .await
            .unwrap();
    }

    async fn atomic_arb(db: &ClickhouseTestClient<BrontesClickhouseTables>) {
        let swap = NormalizedSwap::default();
        let gas_details = GasDetails::default();
        let case0 = AtomicArb { swaps: vec![swap], gas_details, ..AtomicArb::default() };

        db.insert_one::<MevAtomic_Arbs>(&DbDataWithRunId::new_with_run_id(case0, 0))
            .await
            .unwrap();
    }

    async fn pools(db: &ClickhouseTestClient<BrontesClickhouseTables>) {
        let case0 = ProtocolInfoClickhouse {
            protocol: "NONE".to_string(),
            protocol_subtype: "NONE".to_string(),
            address: "0x229b8325bb9Ac04602898B7e8989998710235d5f"
                .to_string()
                .into(),
            tokens: vec!["0x229b8325bb9Ac04602898B7e8989998710235d5f"
                .to_string()
                .into()],
            curve_lp_token: Some(
                "0x229b8325bb9Ac04602898B7e8989998710235d5f"
                    .to_string()
                    .into(),
            ),
            init_block: 0,
        };

        db.insert_one::<EthereumPools>(&case0).await.unwrap();
    }

    async fn block_analysis(db: &ClickhouseTestClient<BrontesClickhouseTables>) {
        let case0 = BlockAnalysis::default();

        db.insert_one::<BrontesBlock_Analysis>(&DbDataWithRunId::new_with_run_id(case0, 0))
            .await
            .unwrap();
    }

    async fn tree(db: &ClickhouseTestClient<BrontesClickhouseTables>) {
        let tree = load_tree().await;

        let roots: Vec<_> = tree
            .tx_roots
            .iter()
            .map(|root| {
                DbDataWithRunId::<TransactionRoot>::new_with_run_id(
                    (root, tree.header.number).into(),
                    0,
                )
            })
            .collect::<Vec<_>>();

        db.insert_many::<BrontesTree>(&roots).await.unwrap();
    }

    async fn run_all(database: &ClickhouseTestClient<BrontesClickhouseTables>) {
        pools(database).await;
        atomic_arb(database).await;
        sandwich(database).await;
        bundle_header(database).await;
        liquidations(database).await;
        jit_sandwich(database).await;
        jit(database).await;
        cex_dex(database).await;
        cex_dex_quotes(database).await;
        mev_block(database).await;
        dex_price_mapping(database).await;
        token_info(database).await;
        tree(database).await;
        block_analysis(database).await;
    }

    #[brontes_macros::test]
    async fn test_all_inserts() {
        init_thread_pools(10);
        let test_db = ClickhouseTestClient { client: Clickhouse::new_default(None).await.client };

        let tables = &BrontesClickhouseTables::all_tables();
        test_db
            .run_test_with_test_db(tables, |db| Box::pin(run_all(db)))
            .await;
    }
}
