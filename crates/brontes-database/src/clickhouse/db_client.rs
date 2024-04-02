#[cfg(not(feature = "cex-dex-markout"))]
use std::cmp::max;
use std::fmt::Debug;

use ::clickhouse::DbRow;
use alloy_primitives::Address;
#[cfg(not(feature = "cex-dex-markout"))]
use brontes_types::db::raw_cex_quotes::{CexQuotesConverter, RawCexQuotes};
#[cfg(feature = "cex-dex-markout")]
use brontes_types::db::raw_cex_trades::{CexTradesConverter, RawCexTrades};
#[cfg(feature = "local-clickhouse")]
use brontes_types::db::{block_times::BlockTimes, cex_symbols::CexSymbols};
use brontes_types::{
    db::{
        address_to_protocol_info::ProtocolInfoClickhouse,
        builder::{BuilderInfo, BuilderInfoWithAddress},
        dex::{DexQuotes, DexQuotesWithBlockNumber},
        metadata::{BlockMetadata, Metadata},
        normalized_actions::TransactionRoot,
        searcher::{JoinedSearcherInfo, SearcherInfo},
        token_info::{TokenInfo, TokenInfoWithAddress},
    },
    mev::{Bundle, BundleData, MevBlock},
    normalized_actions::Actions,
    structured_trace::TxTrace,
    BlockTree, Protocol,
};
use db_interfaces::{
    clickhouse::{client::ClickhouseClient, config::ClickhouseConfig, errors::ClickhouseError},
    Database,
};
use futures::future::join_all;
use serde::Deserialize;

#[cfg(not(feature = "cex-dex-markout"))]
use super::RAW_CEX_QUOTES;
#[cfg(feature = "cex-dex-markout")]
use super::RAW_CEX_TRADES;
use super::{cex_config::CexDownloadConfig, dbms::*, ClickhouseHandle};
#[cfg(feature = "local-clickhouse")]
use super::{BLOCK_TIMES, CEX_SYMBOLS};
#[cfg(feature = "local-clickhouse")]
use crate::libmdbx::cex_utils::CexRangeOrArbitrary;
#[cfg(not(feature = "cex-dex-markout"))]
use crate::libmdbx::{determine_eth_prices, tables::CexPriceData};
use crate::{
    clickhouse::const_sql::BLOCK_INFO,
    libmdbx::{tables::BlockInfoData, types::LibmdbxData},
    CompressedTable,
};

#[derive(Default)]
pub struct Clickhouse {
    client:              ClickhouseClient<BrontesClickhouseTables>,
    cex_download_config: CexDownloadConfig,
}

impl Clickhouse {
    pub fn new(config: ClickhouseConfig, cex_download_config: CexDownloadConfig) -> Self {
        let client = ClickhouseClient::new(config);
        Self { client, cex_download_config }
    }

    pub fn inner(&self) -> &ClickhouseClient<BrontesClickhouseTables> {
        &self.client
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
        searcher_eoa: Address,
        searcher_info: SearcherInfo,
    ) -> eyre::Result<()> {
        let joined = JoinedSearcherInfo::new_eoa(searcher_eoa, searcher_info);

        self.client
            .insert_one::<ClickhouseSearcherInfo>(&joined)
            .await?;

        Ok(())
    }

    pub async fn write_searcher_contract_info(
        &self,
        searcher_contract: Address,
        searcher_info: SearcherInfo,
    ) -> eyre::Result<()> {
        let joined = JoinedSearcherInfo::new_eoa(searcher_contract, searcher_info);

        self.client
            .insert_one::<ClickhouseSearcherInfo>(&joined)
            .await?;

        Ok(())
    }

    pub async fn write_builder_info(
        &self,
        builder_eoa: Address,
        builder_info: BuilderInfo,
    ) -> eyre::Result<()> {
        let info = BuilderInfoWithAddress::new_with_address(builder_eoa, builder_info);

        self.client
            .insert_one::<ClickhouseBuilderInfo>(&info)
            .await?;

        Ok(())
    }

    pub async fn save_mev_blocks(
        &self,
        _block_number: u64,
        block: MevBlock,
        mev: Vec<Bundle>,
    ) -> eyre::Result<()> {
        self.client
            .insert_one::<ClickhouseMevBlocks>(&block)
            .await?;

        let (bundle_headers, bundle_data): (Vec<_>, Vec<_>) = mev
            .into_iter()
            .map(|bundle| (bundle.header, bundle.data))
            .unzip();

        self.client
            .insert_many::<ClickhouseBundleHeader>(&bundle_headers)
            .await?;

        join_all(bundle_data.into_iter().map(|data| async move {
            match data {
                BundleData::Sandwich(s) => {
                    self.client.insert_one::<ClickhouseSandwiches>(&s).await?
                }
                BundleData::AtomicArb(a) => {
                    self.client.insert_one::<ClickhouseAtomicArbs>(&a).await?
                }
                BundleData::JitSandwich(j) => {
                    self.client.insert_one::<ClickhouseJitSandwich>(&j).await?
                }
                BundleData::Jit(j) => self.client.insert_one::<ClickhouseJit>(&j).await?,
                BundleData::CexDex(c) => self.client.insert_one::<ClickhouseCexDex>(&c).await?,
                BundleData::Liquidation(l) => {
                    self.client.insert_one::<ClickhouseLiquidations>(&l).await?
                }
                BundleData::Unknown(u) => {
                    self.client.insert_one::<ClickhouseSearcherTx>(&u).await?
                }
            };

            Ok(())
        }))
        .await
        .into_iter()
        .collect::<Result<Vec<_>, ClickhouseError>>()?;

        Ok(())
    }

    pub async fn write_dex_quotes(
        &self,
        block_num: u64,
        quotes: Option<DexQuotes>,
    ) -> eyre::Result<()> {
        if let Some(q) = quotes {
            let quotes_with_block = DexQuotesWithBlockNumber::new_with_block(block_num, q);

            self.client
                .insert_many::<ClickhouseDexPriceMapping>(&quotes_with_block)
                .await?;
        }

        Ok(())
    }

    pub async fn insert_tree(&self, tree: std::sync::Arc<BlockTree<Actions>>) -> eyre::Result<()> {
        let roots: Vec<TransactionRoot> = tree
            .tx_roots
            .iter()
            .map(|root| (root, tree.header.number).into())
            .collect::<Vec<_>>();

        self.client.insert_many::<ClickhouseTree>(&roots).await?;

        Ok(())
    }

    pub async fn write_token_info(
        &self,
        address: Address,
        decimals: u8,
        symbol: String,
    ) -> eyre::Result<()> {
        self.client
            .insert_one::<ClickhouseTokenInfo>(&TokenInfoWithAddress {
                address,
                inner: TokenInfo { symbol, decimals },
            })
            .await?;

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
        self.client
            .insert_one::<ClickhousePools>(&ProtocolInfoClickhouse::new(
                block,
                address,
                tokens,
                curve_lp_token,
                classifier_name,
            ))
            .await?;

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
            .query_one::<BlockInfoData, _>(BLOCK_INFO, &(block_num))
            .await?
            .value;

        #[cfg(not(feature = "cex-dex-markout"))]
        {
            let cex_quotes_for_block = self
                .get_cex_prices(CexRangeOrArbitrary::Range(block_num, block_num))
                .await?;

            let cex_quotes = cex_quotes_for_block.first().unwrap().clone();

            let eth_prices = determine_eth_prices(&cex_quotes.value);

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
            .into_metadata(cex_quotes.value, None, None))
        }

        #[cfg(feature = "cex-dex-markout")]
        Ok(BlockMetadata::new(
            block_num,
            block_meta.block_hash,
            block_meta.block_timestamp,
            block_meta.relay_timestamp,
            block_meta.p2p_timestamp,
            block_meta.proposer_fee_recipient,
            block_meta.proposer_mev_reward,
            Default::default(),
            block_meta.private_flow.into_iter().collect(),
        )
        .into_metadata(Default::default(), None, None, None))
    }

    async fn query_many_range<T, D>(&self, start_block: u64, end_block: u64) -> eyre::Result<Vec<D>>
    where
        T: CompressedTable,
        T::Value: From<T::DecompressedValue> + Into<T::DecompressedValue>,
        D: LibmdbxData<T> + DbRow + for<'de> Deserialize<'de> + Send + Debug + 'static,
    {
        self.client
            .query_many::<D, _>(
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
        let mut query = T::INIT_QUERY
            .expect("no init query found for clickhouse query")
            .to_string();

        query = query.replace(
            "block_number >= ? AND block_number < ?",
            &format!("block_number IN (SELECT arrayJoin({:?}) AS block_number)", range),
        );

        self.client
            .query_many::<D, _>(&query, &())
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
            .query_many::<D, _>(
                T::INIT_QUERY.expect("no init query found for clickhouse query"),
                &(),
            )
            .await
            .map_err(Into::into)
    }

    fn inner(&self) -> &ClickhouseClient<BrontesClickhouseTables> {
        &self.client
    }

    #[cfg(not(feature = "cex-dex-markout"))]
    async fn get_cex_prices(
        &self,
        range_or_arbitrary: CexRangeOrArbitrary,
    ) -> eyre::Result<Vec<crate::CexPriceData>> {
        let block_times: Vec<BlockTimes> = match range_or_arbitrary {
            CexRangeOrArbitrary::Range(s, e) => {
                self.client.query_many(BLOCK_TIMES, &(s, e)).await?
            }
            CexRangeOrArbitrary::Arbitrary(vals) => {
                let mut query = BLOCK_TIMES.to_string();

                query = query.replace(
                    "block_number >= ? AND block_number < ?",
                    &format!("block_number IN (SELECT arrayJoin({:?}) AS block_number)", vals),
                );

                self.client.query_many(query, &()).await?
            }
        };

        if block_times.is_empty() {
            return Ok(vec![])
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

        let data: Vec<RawCexQuotes> = match range_or_arbitrary {
            CexRangeOrArbitrary::Range(..) => {
                let start_time = block_times
                    .iter()
                    .min_by_key(|b| b.timestamp)
                    .map(|b| b.timestamp)
                    .unwrap()
                    - self.cex_download_config.time_window.0 * 1000;

                let end_time = block_times
                    .iter()
                    .max_by_key(|b| b.timestamp)
                    .map(|b| b.timestamp)
                    .unwrap()
                    + self.cex_download_config.time_window.1 * 1000;

                let query = format!("{RAW_CEX_QUOTES} AND ({exchanges_str})");

                println!("PRICES RANGE: {query}");

                self.client
                    .query_many(query, &(start_time, end_time))
                    .await?
            }
            CexRangeOrArbitrary::Arbitrary(_) => {
                let mut query = RAW_CEX_QUOTES.to_string();

                let query_mod = block_times
                    .iter()
                    .map(|b| b.convert_to_timestamp_query(12000, 0))
                    .collect::<Vec<_>>()
                    .join(" OR ");

                query = query.replace(
                    "timestamp >= ? AND timestamp < ?",
                    &format!("({query_mod}) AND ({exchanges_str})"),
                );

                println!("PRICES ARBITRARY: {query}");

                self.client.query_many(query, &()).await?
            }
        };

        let price_converter = CexQuotesConverter::new(block_times, symbols, data);
        let prices = price_converter
            .convert_to_prices()
            .into_iter()
            .map(|(block_num, price_map)| CexPriceData::new(block_num, price_map))
            .collect();

        Ok(prices)
    }

    #[cfg(feature = "cex-dex-markout")]
    async fn get_cex_trades(
        &self,
        range_or_arbitrary: CexRangeOrArbitrary,
    ) -> eyre::Result<Vec<crate::CexTradesData>> {
        let block_times: Vec<BlockTimes> = match range_or_arbitrary {
            CexRangeOrArbitrary::Range(s, e) => {
                self.client.query_many(BLOCK_TIMES, &(s, e)).await?
            }
            CexRangeOrArbitrary::Arbitrary(vals) => {
                let mut query = BLOCK_TIMES.to_string();

                query = query.replace(
                    "block_number >= ? AND block_number < ?",
                    &format!("block_number IN (SELECT arrayJoin({:?}) AS block_number)", vals),
                );

                self.client.query_many(query, &()).await?
            }
        };

        if block_times.is_empty() {
            return Ok(vec![])
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

        let data: Vec<RawCexTrades> = match range_or_arbitrary {
            CexRangeOrArbitrary::Range(..) => {
                let start_time = block_times
                    .iter()
                    .min_by_key(|b| b.timestamp)
                    .map(|b| b.timestamp)
                    .unwrap()
                    - self.cex_download_config.time_window.0;

                let end_time = block_times
                    .iter()
                    .max_by_key(|b| b.timestamp)
                    .map(|b| b.timestamp)
                    .unwrap()
                    + self.cex_download_config.time_window.1;

                let query = format!("{RAW_CEX_TRADES} AND ({exchanges_str})");

                println!("TRADES RANGE: {query}");

                self.client
                    .query_many(query, &(start_time, end_time))
                    .await?
            }
            CexRangeOrArbitrary::Arbitrary(_) => {
                let mut query = RAW_CEX_TRADES.to_string();

                let query_mod = block_times
                    .iter()
                    .map(|b| b.convert_to_timestamp_query(6000, 6000))
                    .collect::<Vec<_>>()
                    .join(" OR ");

                query = query.replace(
                    "timestamp >= ? AND timestamp < ?",
                    &format!("({query_mod}) AND ({exchanges_str})"),
                );

                println!("TRADES ARBITRARY: {query}");

                self.client.query_many(query, &()).await?
            }
        };

        let trades_converter = CexTradesConverter::new(block_times, symbols, data);
        let trades = trades_converter
            .convert_to_trades()
            .into_iter()
            .map(|(block_num, trade_map)| crate::CexTradesData::new(block_num, trade_map))
            .collect();

        Ok(trades)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use alloy_primitives::hex;
    use brontes_classifier::test_utils::ClassifierTestUtils;
    use brontes_core::{get_db_handle, init_trace_parser};
    use brontes_types::{
        db::{dex::DexPrices, searcher::SearcherEoaContract},
        init_threadpools,
        mev::{
            AtomicArb, BundleHeader, CexDex, JitLiquidity, JitLiquiditySandwich, Liquidation,
            PossibleMev, PossibleMevCollection, Sandwich,
        },
        normalized_actions::{
            NormalizedBurn, NormalizedLiquidation, NormalizedMint, NormalizedSwap,
        },
        pair::Pair,
        FastHashMap, GasDetails,
    };
    use db_interfaces::{
        clickhouse::{dbms::ClickhouseDBMS, test_utils::test_db::ClickhouseTestingClient},
        test_utils::TestDatabase,
    };
    use tokio::sync::mpsc::unbounded_channel;

    use super::*;

    async fn load_tree() -> Arc<BlockTree<Actions>> {
        let classifier_utils = ClassifierTestUtils::new().await;
        let tx = hex!("31dedbae6a8e44ec25f660b3cd0e04524c6476a0431ab610bb4096f82271831b").into();
        classifier_utils.build_tree_tx(tx).await.unwrap().into()
    }

    async fn tx_traces(db: &ClickhouseTestingClient<BrontesClickhouseTables>) {
        let libmdbx = get_db_handle(tokio::runtime::Handle::current()).await;
        let (a, _b) = unbounded_channel();
        let tracer = init_trace_parser(tokio::runtime::Handle::current(), a, libmdbx, 10).await;

        let binding = tracer.execute_block(17000010).await.unwrap();
        let exec = binding.0.first().unwrap().clone();

        let res = db.insert_one::<ClickhouseTxTraces>(&exec).await;
        assert!(res.is_ok());
    }

    async fn searcher_info(db: &ClickhouseTestingClient<BrontesClickhouseTables>) {
        let case0 = JoinedSearcherInfo {
            address:         Default::default(),
            fund:            Default::default(),
            mev:             Default::default(),
            builder:         Some(Default::default()),
            eoa_or_contract: SearcherEoaContract::Contract,
            config_labels:   Default::default(),
            pnl:             Default::default(),
            gas_bids:        Default::default(),
        };

        db.insert_one::<ClickhouseSearcherInfo>(&case0)
            .await
            .unwrap();

        let query = "SELECT * FROM brontes.searcher_info";
        let queried: JoinedSearcherInfo = db.query_one(query, &()).await.unwrap();

        assert_eq!(queried, case0);
    }

    async fn token_info(db: &ClickhouseTestingClient<BrontesClickhouseTables>) {
        let case0 = TokenInfoWithAddress::default();

        db.insert_one::<ClickhouseTokenInfo>(&case0).await.unwrap();
    }

    // async fn searcher_stats(db:
    // &ClickhouseTestingClient<BrontesClickhouseTables>) {     let case0 =
    // SearcherStatsWithAddress::default();
    //
    //     db.insert_one::<ClickhouseSearcherStats>(&case0)
    //         .await
    //         .unwrap();
    //
    //     let query = "SELECT * FROM brontes.searcher_stats";
    //     let queried: SearcherStatsWithAddress = db.query_one(query,
    // &()).await.unwrap();
    //
    //     assert_eq!(queried, case0);
    // }

    async fn builder_stats(_db: &ClickhouseTestingClient<BrontesClickhouseTables>) {
        todo!();
        /*
        let case0 = BuilderStatsWithAddress::default();

        db.insert_one::<ClickhouseBuilderStats>(&case0)
            .await
            .unwrap();

        let query = "SELECT * FROM brontes.builder_stats";
        let queried: BuilderStatsWithAddress = db.query_one(query, &()).await.unwrap();

        assert_eq!(queried, case0);
        */
    }

    async fn dex_price_mapping(db: &ClickhouseTestingClient<BrontesClickhouseTables>) {
        let case0_pair = Pair::default();
        let case0_dex_prices = DexPrices::default();
        let mut case0_map = FastHashMap::default();
        case0_map.insert(case0_pair, case0_dex_prices);

        let case0 = DexQuotesWithBlockNumber {
            block_number: Default::default(),
            tx_idx:       Default::default(),
            quote:        Some(case0_map),
        };

        db.insert_one::<ClickhouseDexPriceMapping>(&case0)
            .await
            .unwrap();

        let query = "SELECT * FROM brontes.dex_price_mapping";
        let queried: DexQuotesWithBlockNumber = db.query_one(query, &()).await.unwrap();

        assert_eq!(queried, case0);
    }

    async fn mev_block(db: &ClickhouseTestingClient<BrontesClickhouseTables>) {
        let case0_possible = PossibleMev::default();
        let case0 = MevBlock {
            possible_mev: PossibleMevCollection(vec![case0_possible]),
            ..Default::default()
        };

        db.insert_one::<ClickhouseMevBlocks>(&case0).await.unwrap();
    }

    async fn cex_dex(db: &ClickhouseTestingClient<BrontesClickhouseTables>) {
        let case0 = CexDex::default();

        db.insert_one::<ClickhouseCexDex>(&case0).await.unwrap();
    }

    async fn jit(db: &ClickhouseTestingClient<BrontesClickhouseTables>) {
        let mut case0 = JitLiquidity::default();
        let swap = NormalizedSwap::default();
        let mint = NormalizedMint::default();
        let burn = NormalizedBurn::default();
        let gas_details = GasDetails::default();

        case0.frontrun_mints = vec![mint];
        case0.backrun_burns = vec![burn];
        case0.victim_swaps = vec![vec![swap]];
        case0.victim_swaps_gas_details = vec![gas_details];

        db.insert_one::<ClickhouseJit>(&case0).await.unwrap();
    }

    async fn jit_sandwich(db: &ClickhouseTestingClient<BrontesClickhouseTables>) {
        let mut case0 = JitLiquiditySandwich::default();
        let swap = NormalizedSwap::default();
        let mint = NormalizedMint::default();
        let burn = NormalizedBurn::default();
        let gas_details = GasDetails::default();

        case0.frontrun_mints = vec![Some(vec![mint])];
        case0.backrun_burns = vec![burn];
        case0.victim_swaps = vec![vec![swap]];
        case0.victim_swaps_gas_details = vec![gas_details];

        db.insert_one::<ClickhouseJitSandwich>(&case0)
            .await
            .unwrap();
    }

    async fn liquidations(db: &ClickhouseTestingClient<BrontesClickhouseTables>) {
        let mut case0 = Liquidation::default();
        let swap = NormalizedSwap::default();
        let liquidation = NormalizedLiquidation::default();
        let gas_details = GasDetails::default();

        case0.liquidation_swaps = vec![swap];
        case0.liquidations = vec![liquidation];
        case0.gas_details = gas_details;

        db.insert_one::<ClickhouseLiquidations>(&case0)
            .await
            .unwrap();
    }

    async fn bundle_header(db: &ClickhouseTestingClient<BrontesClickhouseTables>) {
        let case0 = BundleHeader::default();

        db.insert_one::<ClickhouseBundleHeader>(&case0)
            .await
            .unwrap();
    }

    async fn sandwich(db: &ClickhouseTestingClient<BrontesClickhouseTables>) {
        let mut case0 = Sandwich::default();
        let swap0 = NormalizedSwap::default();
        let swap1 = NormalizedSwap::default();
        let swap2 = NormalizedSwap::default();
        let gas_details = GasDetails::default();

        case0.frontrun_swaps = vec![vec![swap0]];
        case0.victim_swaps = vec![vec![swap1]];
        case0.victim_swaps_gas_details = vec![gas_details];
        case0.backrun_swaps = vec![swap2];

        db.insert_one::<ClickhouseSandwiches>(&case0).await.unwrap();
    }

    async fn atomic_arb(db: &ClickhouseTestingClient<BrontesClickhouseTables>) {
        let mut case0 = AtomicArb::default();
        let swap = NormalizedSwap::default();
        let gas_details = GasDetails::default();

        case0.swaps = vec![swap];
        case0.gas_details = gas_details;

        db.insert_one::<ClickhouseAtomicArbs>(&case0).await.unwrap();
    }

    async fn pools(db: &ClickhouseTestingClient<BrontesClickhouseTables>) {
        let case0 = ProtocolInfoClickhouse {
            protocol:         "NONE".to_string(),
            protocol_subtype: "NONE".to_string(),
            address:          "0x229b8325bb9Ac04602898B7e8989998710235d5f"
                .to_string()
                .into(),
            tokens:           vec!["0x229b8325bb9Ac04602898B7e8989998710235d5f"
                .to_string()
                .into()],
            curve_lp_token:   Some(
                "0x229b8325bb9Ac04602898B7e8989998710235d5f"
                    .to_string()
                    .into(),
            ),
            init_block:       0,
        };

        db.insert_one::<ClickhousePools>(&case0).await.unwrap();
    }

    async fn builder_info(db: &ClickhouseTestingClient<BrontesClickhouseTables>) {
        let case0 = BuilderInfoWithAddress::default();

        db.insert_one::<ClickhouseBuilderInfo>(&case0)
            .await
            .unwrap();
    }

    async fn tree(db: &ClickhouseTestingClient<BrontesClickhouseTables>) {
        let tree = load_tree().await;

        let roots: Vec<TransactionRoot> = tree
            .tx_roots
            .iter()
            .map(|root| (root, tree.header.number).into())
            .collect::<Vec<_>>();

        db.insert_many::<ClickhouseTree>(&roots).await.unwrap();
    }

    async fn run_all(database: &ClickhouseTestingClient<BrontesClickhouseTables>) {
        tx_traces(database).await;
        builder_info(database).await;
        pools(database).await;
        atomic_arb(database).await;
        sandwich(database).await;
        bundle_header(database).await;
        liquidations(database).await;
        jit_sandwich(database).await;
        jit(database).await;
        cex_dex(database).await;
        mev_block(database).await;
        dex_price_mapping(database).await;
        builder_stats(database).await;
        // searcher_stats(database).await;
        token_info(database).await;
        searcher_info(database).await;
        tree(database).await;
    }

    #[brontes_macros::test]
    async fn test_all_inserts() {
        dotenv::dotenv().ok();
        init_threadpools(10);
        let test_db = ClickhouseTestingClient::<BrontesClickhouseTables>::default();

        let tables = &BrontesClickhouseTables::all_tables();

        test_db
            .run_test_with_test_db(tables, |db| Box::pin(run_all(db)))
            .await;
    }
}
