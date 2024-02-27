use std::{cmp::max, fmt::Debug};

use ::clickhouse::DbRow;
use alloy_primitives::Address;
use brontes_types::{
    db::{
        address_to_protocol_info::ProtocolInfoClickhouse,
        builder::{BuilderInfo, BuilderInfoWithAddress, BuilderStats, BuilderStatsWithAddress},
        dex::{DexQuotes, DexQuotesWithBlockNumber},
        metadata::{BlockMetadata, Metadata},
        searcher::{JoinedSearcherInfo, SearcherInfo, SearcherStats, SearcherStatsWithAddress},
        token_info::{TokenInfo, TokenInfoWithAddress},
    },
    mev::{Bundle, BundleData, MevBlock},
    structured_trace::TxTrace,
    Protocol,
};
use futures::future::join_all;
use serde::Deserialize;
use sorella_db_databases::{
    clickhouse::{config::ClickhouseConfig, db::ClickhouseClient, errors::ClickhouseError},
    Database,
};

use super::{dbms::*, ClickhouseHandle};
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

    pub async fn write_searcher_stats(
        &self,
        searcher_eoa: Address,
        searcher_stats: SearcherStats,
    ) -> eyre::Result<()> {
        let stats = SearcherStatsWithAddress::new_with_address(searcher_eoa, searcher_stats);

        self.client
            .insert_one::<ClickhouseSearcherStats>(&stats)
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

    pub async fn write_builder_stats(
        &self,
        builder_eoa: Address,
        builder_stats: BuilderStats,
    ) -> eyre::Result<()> {
        let stats = BuilderStatsWithAddress::new_with_address(builder_eoa, builder_stats);

        self.client
            .insert_one::<ClickhouseBuilderStats>(&stats)
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
                BundleData::Unknown => (),
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
            .query_many::<D>(T::INIT_QUERY.expect("no init query found for clickhouse query"), &())
            .await
            .map_err(Into::into)
    }

    fn inner(&self) -> &ClickhouseClient<BrontesClickhouseTables> {
        &self.client
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use brontes_core::{get_db_handle, init_trace_parser};
    use brontes_types::{
        db::{
            dex::DexPrices,
            searcher::{SearcherEoaContract, SearcherStatsWithAddress},
        },
        mev::{
            AtomicArb, BundleHeader, CexDex, JitLiquidity, JitLiquiditySandwich, Liquidation,
            MevType, PossibleMev, PossibleMevCollection, Sandwich,
        },
        normalized_actions::{
            NormalizedBurn, NormalizedLiquidation, NormalizedMint, NormalizedSwap,
        },
        pair::Pair,
        GasDetails,
    };
    use sorella_db_databases::{
        clickhouse::test_utils::test_db::ClickhouseTestingClient, test_utils::TestDatabase,
    };
    use tokio::sync::mpsc::unbounded_channel;

    use super::*;
    use crate::clickhouse::dbms::{
        ClickhouseBundleHeader, ClickhouseCexDex, ClickhouseJit, ClickhouseJitSandwich,
        ClickhouseLiquidations, ClickhouseMevBlocks, ClickhouseSearcherStats,
    };

    fn spawn_clickhouse() -> ClickhouseTestingClient<BrontesClickhouseTables> {
        dotenv::dotenv().ok();

        ClickhouseTestingClient::default()
    }

    #[brontes_macros::test]
    async fn tx_traces() {
        let db = spawn_clickhouse();

        let libmdbx = get_db_handle(tokio::runtime::Handle::current()).await;
        let (a, _b) = unbounded_channel();
        let tracer = init_trace_parser(tokio::runtime::Handle::current(), a, libmdbx, 10).await;

        let binding = tracer.execute_block(17000010).await.unwrap();
        let exec = binding.0.first().unwrap().clone();

        let res = db.insert_one::<ClickhouseTxTraces>(&exec).await;
        assert!(res.is_ok());
    }

    #[tokio::test]
    async fn searcher_info() {
        let db = spawn_clickhouse();
        let case0 = JoinedSearcherInfo {
            address:         Default::default(),
            fund:            Default::default(),
            mev:             vec![MevType::default()],
            builder:         Some(Default::default()),
            eoa_or_contract: SearcherEoaContract::Contract,
        };

        db.insert_one::<ClickhouseSearcherInfo>(&case0)
            .await
            .unwrap();

        let query = "SELECT * FROM brontes.searcher_info";
        let queried: JoinedSearcherInfo = db.query_one(query, &()).await.unwrap();

        assert_eq!(queried, case0);
    }

    #[tokio::test]
    async fn token_info() {
        let db = spawn_clickhouse();
        let case0 = TokenInfoWithAddress::default();

        db.insert_one::<ClickhouseTokenInfo>(&case0).await.unwrap();

        //let query = "SELECT address, (decimals, symbol) FROM
        // brontes.token_info WHERE address =
        // '0x0000000000000000000000000000000000000000'"; let queried:
        // TokenInfoWithAddress = db.query_one(query,
        // &()).await.unwrap();

        //assert_eq!(queried, case0);

        //let query = "DELETE FROM brontes.token_info WHERE address =
        // '0x0000000000000000000000000000000000000000'"; db.
        // execute_remote(query, &()).await.unwrap();
    }

    #[tokio::test]
    async fn searcher_stats() {
        let db = spawn_clickhouse();
        let case0 = SearcherStatsWithAddress::default();

        db.insert_one::<ClickhouseSearcherStats>(&case0)
            .await
            .unwrap();

        let query = "SELECT * FROM brontes.searcher_stats";
        let queried: SearcherStatsWithAddress = db.query_one(query, &()).await.unwrap();

        assert_eq!(queried, case0);
    }

    #[tokio::test]
    async fn builder_stats() {
        let db = spawn_clickhouse();
        let case0 = BuilderStatsWithAddress::default();

        db.insert_one::<ClickhouseBuilderStats>(&case0)
            .await
            .unwrap();

        let query = "SELECT * FROM brontes.builder_stats";
        let queried: BuilderStatsWithAddress = db.query_one(query, &()).await.unwrap();

        assert_eq!(queried, case0);
    }

    #[tokio::test]
    async fn dex_price_mapping() {
        let db = spawn_clickhouse();

        let case0_pair = Pair::default();
        let case0_dex_prices = DexPrices::default();
        let mut case0_map = HashMap::new();
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
    #[allow(clippy::field_reassign_with_default)]
    #[tokio::test]
    async fn mev_block() {
        let db = spawn_clickhouse();

        let case0_possible = PossibleMev::default();
        let mut case0 = MevBlock::default();

        case0.possible_mev = PossibleMevCollection(vec![case0_possible]);

        db.insert_one::<ClickhouseMevBlocks>(&case0).await.unwrap();
    }

    #[tokio::test]
    async fn cex_dex() {
        let db = spawn_clickhouse();

        let case0 = CexDex::default();

        db.insert_one::<ClickhouseCexDex>(&case0).await.unwrap();
    }

    #[tokio::test]
    async fn jit() {
        let db = spawn_clickhouse();

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

    #[tokio::test]
    async fn jit_sandwich() {
        let db = spawn_clickhouse();

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

    #[tokio::test]
    async fn liquidations() {
        let db = spawn_clickhouse();

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

    #[tokio::test]
    async fn bundle_header() {
        let db = spawn_clickhouse();

        let case0 = BundleHeader::default();

        db.insert_one::<ClickhouseBundleHeader>(&case0)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn sandwich() {
        let db = spawn_clickhouse();

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

    /// runs all the tests
    async fn run_all(database: ClickhouseTestingClient<BrontesClickhouseTables>) {
        atomic_arb(&database).await;
        panic!("STOP")
    }

    #[tokio::test]
    /// runs all the tests
    async fn test_all_inserts() {
        let tables = &[BrontesClickhouseTables::ClickhouseAtomicArbs];

        let test_db: ClickhouseTestingClient<BrontesClickhouseTables> = spawn_clickhouse();
        test_db
            .run_test_with_test_db(tables, |db| Box::pin(run_all(db)))
            .await;
    }

    async fn atomic_arb(db: &ClickhouseTestingClient<BrontesClickhouseTables>) {
        //let tables = [BrontesClickhouseTables::ClickhouseAtomicArbs];

        let mut case0 = AtomicArb::default();
        let swap = NormalizedSwap::default();
        let gas_details = GasDetails::default();

        case0.swaps = vec![swap];
        case0.gas_details = gas_details;

        db.insert_one::<ClickhouseAtomicArbs>(&case0).await.unwrap();
    }

    #[tokio::test]
    async fn pools() {
        let db = spawn_clickhouse();

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

    #[tokio::test]
    async fn builder_info() {
        let db = spawn_clickhouse();

        let case0 = BuilderInfoWithAddress::default();

        db.insert_one::<ClickhouseBuilderInfo>(&case0)
            .await
            .unwrap();
    }
    #[allow(dead_code)]
    #[allow(non_upper_case_globals)]
    const queryy: &str = "SELECT
    block_number,
    groupArray(
        (
            block_number,
            arrayZip(
                trace_meta.trace_idx,
                trace_meta.msg_sender,
                trace_meta.error,
                trace_meta.subtraces,
                trace_meta.trace_address
            ),
            arrayZip(
                trace_decoded_data.trace_idx,
                trace_decoded_data.function_name,
                trace_decoded_data.call_data,
                trace_decoded_data.return_data
            ),
            arrayZip(
                trace_logs.trace_idx,
                trace_logs.log_idx,
                trace_logs.address,
                trace_logs.topics,
                trace_logs.data
            ),
            arrayZip(
                trace_create_actions.trace_idx,
                trace_create_actions.from,
                trace_create_actions.gas,
                trace_create_actions.init,
                trace_create_actions.value
            ),
            arrayZip(
                trace_call_actions.trace_idx,
                trace_call_actions.from,
                trace_call_actions.call_type,
                trace_call_actions.gas,
                trace_call_actions.input,
                trace_call_actions.to,
                trace_call_actions.value
            ),
            arrayZip(
                trace_self_destruct_actions.trace_idx,
                trace_self_destruct_actions.address,
                trace_self_destruct_actions.balance,
                trace_self_destruct_actions.refund_address
            ),
            arrayZip(
                trace_reward_actions.trace_idx,
                trace_reward_actions.author,
                trace_reward_actions.reward_type,
                trace_reward_actions.value
            ),
            arrayZip(
                trace_call_outputs.trace_idx,
                trace_call_outputs.gas_used,
                trace_call_outputs.output
            ),
            arrayZip(
                trace_create_outputs.trace_idx,
                trace_create_outputs.address,
                trace_create_outputs.code,
                trace_create_outputs.gas_used
            ),

        tx_hash,
        gas_used,
        effective_price,
        tx_index,
        is_success
    ))
FROM brontes.tx_traces
WHERE block_number = 15697312
GROUP BY block_number
";
}
