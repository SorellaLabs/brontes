pub mod const_sql;
pub mod errors;
pub mod types;
use std::collections::HashMap;

use alloy_json_abi::JsonAbi;
use brontes_types::classified_mev::{ClassifiedMev, MevBlock, MevType, SpecificMev, *};
use futures::future::join_all;
use reth_primitives::{hex, revm_primitives::FixedBytes, Address};
use sorella_db_databases::{
    clickhouse::{ClickhouseClient, Credentials},
    config::ClickhouseConfig,
    utils::format_query_array,
    Row, BACKRUN_TABLE, CEX_DEX_TABLE, CLASSIFIED_MEV_TABLE, JIT_SANDWICH_TABLE, JIT_TABLE,
    LIQUIDATIONS_TABLE, MEV_BLOCKS_TABLE, SANDWICH_TABLE,
};
use tracing::{error, info};

use self::types::{Abis, DBTokenPricesDB, PoolReservesDB, TimesFlow};
use super::Metadata;
use crate::{
    cex::CexPriceMap,
    database::{const_sql::*, types::TimesFlowDB},
    CexQuote, DexQuote, DexQuotesMap, Pair, PriceGraph,
};

pub const WETH_ADDRESS: Address =
    Address(FixedBytes(hex!("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2")));
pub const USDT_ADDRESS: Address =
    Address(FixedBytes(hex!("dac17f958d2ee523a2206206994597c13d831ec7")));

pub struct Database {
    client: ClickhouseClient,
}

impl Default for Database {
    fn default() -> Self {
        Self { client: ClickhouseClient::default() }
    }
}

impl Database {
    pub fn new(config: ClickhouseConfig) -> Self {
        let client = ClickhouseClient::new(config);
        Self { client }
    }

    pub fn credentials(&self) -> Credentials {
        self.client.credentials()
    }

    pub async fn get_metadata(&self, block_num: u64) -> Metadata {
        let times_flow = self.get_times_flow_info(block_num).await;
        let cex_prices = CexPriceMap::from(self.get_cex_token_prices(times_flow.p2p_time).await);

        //TODO: you were calling clickhouse, so now just making it empty here
        let dex_prices = PriceGraph::from_quotes(DexQuotesMap::<DexQuote>::new());

        // eth price is in cex_prices
        let eth_prices = cex_prices
            .get_quote(&Pair(WETH_ADDRESS, USDT_ADDRESS))
            .unwrap()
            .clone();

        let metadata = Metadata::new(
            block_num,
            times_flow.block_hash.into(),
            times_flow.relay_time,
            times_flow.p2p_time,
            times_flow.proposer_addr,
            times_flow.proposer_reward,
            cex_prices,
            dex_prices,
            eth_prices.avg(),
            times_flow.private_flow,
        );

        metadata
    }

    async fn insert_singe_classified_data<T: SpecificMev + serde::Serialize + Row + Clone>(
        db_client: &ClickhouseClient,
        mev_detail: Box<dyn SpecificMev>,
        table: &str,
    ) {
        let any = mev_detail.into_any();
        let this = any.downcast_ref::<T>().unwrap();
        if let Err(e) = db_client.insert_one(this.clone(), table).await {
            error!(?e, "failed to insert specific mev");
        }
    }

    pub async fn insert_classified_data(
        &self,
        block_details: MevBlock,
        mev_details: Vec<(ClassifiedMev, Box<dyn SpecificMev>)>,
    ) {
        if let Err(e) = self
            .client
            .insert_one(block_details, MEV_BLOCKS_TABLE)
            .await
        {
            error!(?e, "failed to insert block details");
        }

        info!("inserted block details");

        let db_client = &self.client;
        join_all(mev_details.into_iter().map(|(classified, specific)| async {
            if let Err(e) = self
                .client
                .insert_one(classified, CLASSIFIED_MEV_TABLE)
                .await
            {
                error!(?e, "failed to insert classified mev");
            }

            info!("inserted classified_mev");
            let table = &mev_table_type(&specific);
            let mev_type = specific.mev_type();
            match mev_type {
                MevType::Sandwich => {
                    Self::insert_singe_classified_data::<Sandwich>(db_client, specific, table).await
                }
                MevType::Backrun => {
                    Self::insert_singe_classified_data::<AtomicBackrun>(db_client, specific, table)
                        .await
                }
                MevType::JitSandwich => {
                    Self::insert_singe_classified_data::<JitLiquiditySandwich>(
                        db_client, specific, table,
                    )
                    .await
                }
                MevType::Jit => {
                    Self::insert_singe_classified_data::<JitLiquidity>(db_client, specific, table)
                        .await
                }
                MevType::CexDex => {
                    Self::insert_singe_classified_data::<CexDex>(db_client, specific, table).await
                }
                MevType::Liquidation => {
                    Self::insert_singe_classified_data::<Liquidation>(db_client, specific, table)
                        .await
                }
                MevType::Unknown => unimplemented!("none yet"),
            };

            info!(%table,"inserted specific mev type");
        }))
        .await;
    }

    pub async fn get_abis(&self, addresses: Vec<Address>) -> HashMap<Address, JsonAbi> {
        let query = format_query_array(&addresses, ABIS);

        self.client
            .query_all::<Abis>(&query)
            .await
            .unwrap()
            .into_iter()
            .map(Into::into)
            .collect()
    }

    /*
       async fn get_private_flow(&self, block_num: u64) -> HashSet<TxHash> {
           let private_txs = self
               .client
               .query_all_params::<u64, String>(PRIVATE_FLOW, vec![block_num])
               .await
               .unwrap();

           private_txs
               .into_iter()
               .map(|tx| TxHash::from_str(&tx).unwrap())
               .collect::<HashSet<TxHash>>()
       }
    */
    async fn get_times_flow_info(&self, block_num: u64) -> TimesFlow {
        self.client
            .query_one_params::<u64, TimesFlowDB>(TIMES_FLOW, vec![block_num])
            .await
            .unwrap()
            .into()
    }

    async fn get_cex_token_prices(&self, p2p_time: u64) -> CexPriceMap {
        self.client
            .query_all_params::<u64, DBTokenPricesDB>(PRICES, vec![p2p_time])
            .await
            .unwrap()
            .into()
    }
}

fn mev_table_type(mev: &Box<dyn SpecificMev>) -> String {
    match mev.mev_type() {
        brontes_types::classified_mev::MevType::Sandwich => SANDWICH_TABLE,
        brontes_types::classified_mev::MevType::Backrun => BACKRUN_TABLE,
        brontes_types::classified_mev::MevType::JitSandwich => JIT_SANDWICH_TABLE,
        brontes_types::classified_mev::MevType::Jit => JIT_TABLE,
        brontes_types::classified_mev::MevType::CexDex => CEX_DEX_TABLE,
        brontes_types::classified_mev::MevType::Liquidation => LIQUIDATIONS_TABLE,
        brontes_types::classified_mev::MevType::Unknown => "",
    }
    .to_string()
}

#[cfg(test)]
mod tests {

    use std::collections::HashSet;

    use dotenv::dotenv;
    use reth_primitives::{Address, B256};

    use super::*;
    use crate::Quote;

    const BLOCK_NUMBER: u64 = 18180900;
    const BLOCK_HASH: &str = "0x2c6bb65135fd200b7bb92fc9e63017d26a61a34d8ccdb6f6a501dc73bc32ce41";

    fn expected_private_flow() -> HashSet<B256> {
        let set = vec![
            B256::from_str("0x6343399486888e07485ea91f1096b55f5508767df88d928376094318a346bc81")
                .unwrap(),
            B256::from_str("0x4ca5bf9ace6088f1c58cd5a0bbf5830864c4c344b39046ad288775e7e94e58de")
                .unwrap(),
            B256::from_str("0xd328a2afe816f680c495545373fb2d739829c56c88714e9f34326e73ccfe54f2")
                .unwrap(),
            B256::from_str("0x94fee12653f619bb8c7fcec60eb995d50c7430f497583acb45486362973ab1d3")
                .unwrap(),
            B256::from_str("0xc78bf07b9b7b43058e11bb0da08ca29cc9143374aec3a129bef08007f1204477")
                .unwrap(),
            B256::from_str("0xa8cacf2de6e06b83372bf8e1e5264df28d1b810125e6b124cf7a95dfabaeda16")
                .unwrap(),
            B256::from_str("0x89f1c8ae2820654dd03735802f85114f910b3cb63ec8e7408c5464588097a7b4")
                .unwrap(),
            B256::from_str("0x95b67f167abb03b585c2aedb636843797fd02511449dd614a15a772840a3d039")
                .unwrap(),
            B256::from_str("0xb1e92797d49c003f4ee933dd4529ee63568057243ee0dbc4c3ff9f1eb58d6f95")
                .unwrap(),
            B256::from_str("0x75a9c74e6e50adf5debf2e83254d0da58314eb07eefc2ae25718df66b480bb6f")
                .unwrap(),
            B256::from_str("0xdab644a36ffbcdbba36f4950db373ea7377d3a6204a639561e2a1f9a59cbef3f")
                .unwrap(),
            B256::from_str("0x2d43c26e7e7d01cd5e139e710467fb1045ae5d5a5be4bfbaa4b0dee07bdb5edb")
                .unwrap(),
            B256::from_str("0xd79a2c1b9dd8d55ea05eb3653c572c827951fb40f52ff7562801a8b8cca484e0")
                .unwrap(),
            B256::from_str("0x17eb3b75fbf381ac2ef8840763c93363b962255660568194b8da5f6c7aefdc3c")
                .unwrap(),
            B256::from_str("0xf95dfb7950d6cf3cf4aa342239d823d2e77916baf48100bb961d2dfaea63cd49")
                .unwrap(),
            B256::from_str("0xf5de99a8d45aa1b65138b62ed7f77de75efac2d934b6903a3a3927ae4fa9d252")
                .unwrap(),
            B256::from_str("0xce29f9146afd9e954c0f60b0b05b6ede02b73df6512ff0977fb748c72a1ffebd")
                .unwrap(),
            B256::from_str("0xf4e4b9faf3801718b9f1a95ec39970b1831c6efd3dd57c14cbfd77a1e9595802")
                .unwrap(),
            B256::from_str("0xf1b1ab5fcd494071a41764a3847542dfa94251dd81909ea2525a29038d887d5f")
                .unwrap(),
            B256::from_str("0xbf8c489f02b769046452abae13a6d24d2793f0a098937c2db9f71828f0254b81")
                .unwrap(),
            B256::from_str("0x9f18e09fd33378e75f5266b70201d0a7e63d8e7759c571c0a1fe5b3acc83ed7a")
                .unwrap(),
            B256::from_str("0xf7adf7b12196f424b5845b8a56701b06bc8a7f06ed53aa455e41b05b3a4f4a94")
                .unwrap(),
            B256::from_str("0x18484a968cac9ea2dfb9d357d7e14642f474ebfaa447d4812488ae77af6f61e4")
                .unwrap(),
        ];

        HashSet::from_iter(set.into_iter())
    }

    fn is_valid_utf8(s: &str) -> bool {
        let bytes = s.as_bytes();
        std::str::from_utf8(bytes).is_ok()
    }

    fn expected_relay_info() -> TimesFlow {
        TimesFlow {
            relay_time:      1695258707711,
            p2p_time:        1695258708673,
            proposer_addr:   Address::from_str("0x388C818CA8B9251b393131C08a736A67ccB19297")
                .unwrap(),
            proposer_reward: 113949354337187568,
            block_hash:      B256::from_str(BLOCK_HASH).unwrap().into(),
            private_flow:    expected_private_flow(),
            block_number:    BLOCK_NUMBER,
        }
    }

    fn expected_metadata(cex_prices: Quotes) -> Metadata {
        let mut cex_prices = cex_prices.clone();
        let eth_prices = cex_prices
            .get_quote(&Pair(WETH_ADDRESS, USDT_ADDRESS))
            .unwrap()
            .clone();

        Metadata {
            block_num:              BLOCK_NUMBER,
            block_hash:             B256::from_str(BLOCK_HASH).unwrap().into(),
            relay_timestamp:        1695258707711,
            p2p_timestamp:          1695258708673,
            proposer_fee_recipient: Address::from_str("0x388C818CA8B9251b393131C08a736A67ccB19297")
                .unwrap(),
            proposer_mev_reward:    113949354337187568,
            cex_quotes:             cex_prices.clone(),
            eth_prices:             eth_prices.avg().clone(),
            mempool_flow:           expected_private_flow(),
        }
    }

    #[test]
    fn test_valid_utf8() {
        let mut query = TIMES_FLOW.to_string();
        query = query.replace("?", &BLOCK_NUMBER.to_string());

        assert!(is_valid_utf8(&query))
    }

    #[tokio::test]
    async fn test_get_times_flow_info() {
        dotenv().ok();

        let db = Database::default();

        let expected_relay_info = expected_relay_info();
        let relay_info = db.get_times_flow_info(BLOCK_NUMBER).await;

        assert_eq!(expected_relay_info.relay_time, relay_info.relay_time);
        assert_eq!(expected_relay_info.p2p_time, relay_info.p2p_time);
        assert_eq!(expected_relay_info.proposer_addr, relay_info.proposer_addr);
        assert_eq!(expected_relay_info.proposer_reward, relay_info.proposer_reward);
    }

    #[tokio::test]
    async fn test_get_token_prices() {
        dotenv().ok();

        let db = Database::default();

        let cex_prices = db.get_token_prices(1695258708673).await;

        let real_prices = cex_prices
            .get_quote(&Pair(
                Address::from_str("0xaea46a60368a7bd060eec7df8cba43b7ef41ad85").unwrap(),
                Address::from_str("0xb8c77482e45f1f44de1745f52c74426c631bdd52").unwrap(),
            ))
            .unwrap()
            .clone();

        let queried_prices = Quote {
            timestamp: 1695258689127,
            price:     (
                Rational::try_from(0.001072).unwrap(),
                Rational::try_from(0.00107).unwrap(),
            ),
        };
        assert_eq!(real_prices, queried_prices);

        let real_prices = cex_prices
            .get_quote(&Pair(
                Address::from_str("0x86fa049857e0209aa7d9e616f7eb3b3b78ecfdb0").unwrap(),
                Address::from_str("0xb8c77482e45f1f44de1745f52c74426c631bdd52").unwrap(),
            ))
            .unwrap()
            .clone();

        let queried_prices = Quote {
            timestamp: 1695258675246,
            price:     (
                Rational::try_from(0.002715).unwrap(),
                Rational::try_from(0.002709).unwrap(),
            ),
        };

        assert_eq!(real_prices, queried_prices);

        assert_eq!(cex_prices.0.len(), 254);
    }

    #[tokio::test]
    async fn test_get_metadata() {
        dotenv().ok();

        let db = Database::default();

        let cex_prices = db.get_token_prices(1695258708673).await;

        let expected_metadata = expected_metadata(cex_prices);

        let metadata = db.get_metadata(BLOCK_NUMBER).await;

        assert_eq!(metadata.block_num, expected_metadata.block_num);
        assert_eq!(metadata.block_hash, expected_metadata.block_hash);
        assert_eq!(metadata.relay_timestamp, expected_metadata.relay_timestamp);
        assert_eq!(metadata.p2p_timestamp, expected_metadata.p2p_timestamp);
        assert_eq!(metadata.proposer_fee_recipient, expected_metadata.proposer_fee_recipient);
        assert_eq!(metadata.proposer_mev_reward, expected_metadata.proposer_mev_reward);
        assert_eq!(metadata.mempool_flow, expected_metadata.mempool_flow);
    }
}
