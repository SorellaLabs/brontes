mod const_sql;
pub mod errors;
mod middleware;
use alloy_primitives::Address;
use brontes_types::{
    constants::{USDT_ADDRESS, WETH_ADDRESS},
    db::{
        cex::CexPriceMap, clickhouse::*, dex::DexQuotes, metadata::Metadata, searcher::SearcherInfo,
    },
    mev::{Bundle, BundleData, Mev, MevBlock},
    pair::Pair,
    structured_trace::TxTrace,
    Protocol,
};
pub use middleware::*;
use sorella_db_databases::{
    clickhouse::{
        config::ClickhouseConfig, db::ClickhouseClient, utils::format_query_array, Credentials,
    },
    tables::DatabaseTables,
    Database,
};

pub use self::const_sql::*;

#[derive(Default)]
pub struct Clickhouse {
    client: ClickhouseClient,
}

impl Clickhouse {
    pub fn new(config: ClickhouseConfig) -> Self {
        let client = ClickhouseClient::new(config);
        Self { client }
    }

    pub fn inner(&self) -> &ClickhouseClient {
        &self.client
    }

    pub async fn get_metadata(&self, block_num: u64) -> Metadata {
        let times_flow = self.get_times_flow_info(block_num).await;
        let cex_prices = self.get_cex_token_prices(times_flow.p2p_time).await;

        // eth price is in cex_prices
        let _eth_prices = cex_prices
            .get_binance_quote(&Pair(WETH_ADDRESS, USDT_ADDRESS))
            .unwrap()
            .clone();

        /*
        let metadata = MetadataNoDex::new(
            block_num,
            times_flow.block_hash.into(),
            times_flow.relay_time,
            times_flow.p2p_time,
            times_flow.proposer_addr,
            times_flow.proposer_reward,
            cex_prices,
            eth_prices.avg(),
            times_flow.private_flow,
        );

        metadata
         */

        Default::default()
    }

    async fn get_times_flow_info(&self, block_num: u64) -> ClickhouseTimesFlow {
        self.client
            .query_one::<ClickhouseTimesFlow>(TIMES_FLOW, &(block_num))
            .await
            .unwrap()
    }

    async fn get_cex_token_prices(&self, _p2p_time: u64) -> CexPriceMap {
        CexPriceMap::default()

        /*self.client
        .query_many::<ClickhouseTokenPrices>(PRICES, &(p2p_time))
        .await
        .unwrap()
        .into()
        */
    }

    // inserts
    async fn write_searcher_info(
        &self,
        searcher_eoa: Address,
        searcher_info: SearcherInfo,
    ) -> eyre::Result<()> {
        Ok(())
    }

    async fn save_mev_blocks(
        &self,
        block_number: u64,
        block: MevBlock,
        mev: Vec<Bundle>,
    ) -> eyre::Result<()> {
        Ok(())
    }

    async fn write_dex_quotes(
        &self,
        block_num: u64,
        quotes: Option<DexQuotes>,
    ) -> eyre::Result<()> {
        Ok(())
    }

    async fn write_token_info(
        &self,
        address: Address,
        decimals: u8,
        symbol: String,
    ) -> eyre::Result<()> {
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
        Ok(())
    }
}
