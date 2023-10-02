pub mod const_sql;
pub mod errors;
pub mod types;
use std::{
    collections::{HashMap, HashSet},
    str::FromStr,
};

use futures::future::join_all;
use malachite::Rational;
use poirot_types::classified_mev::{ClassifiedMev, MevBlock, SpecificMev};
use reth_primitives::{Address, TxHash, U256};
use serde::Deserialize;
use sorella_db_clients::databases::clickhouse::{self, ClickhouseClient, Row};

use self::types::{DBTokenPrices, RelayInfo};
use super::Metadata;
use crate::database::const_sql::*;

pub struct Database {
    client: ClickhouseClient,
}

impl Default for Database {
    fn default() -> Self {
        Self { client: ClickhouseClient::default() }
    }
}

impl Database {
    pub async fn get_metadata(&self, block_num: u64, block_hash: U256) -> Metadata {
        let private_flow = self.get_private_flow(block_num, block_hash).await;
        let relay_data = self.get_relay_info(block_num, block_hash).await;
        let cex_prices = self
            .get_cex_prices(relay_data.relay_time, relay_data.p2p_time)
            .await;

        // eth price is in cex_prices
        let eth_prices = Default::default();
        // = cex_prices.get("ETH").unwrap();
        // cex_prices.remove("ETH");

        let metadata = Metadata::new(
            block_num,
            block_hash,
            relay_data.relay_time,
            relay_data.p2p_time,
            relay_data.proposer_addr,
            relay_data.proposer_reward,
            cex_prices,
            eth_prices,
            private_flow,
        );

        metadata
    }

    pub async fn insert_classified_data(
        &self,
        block_details: MevBlock,
        mev_details: Vec<(ClassifiedMev, Box<dyn SpecificMev>)>,
    ) {
        // insert block
        self.client
            .insert_one(block_details, "mev.mev_blocks")
            .await;

        join_all(
            mev_details
                .into_iter()
                .map(|(classified, specific)| async {
                }),
        )
        .await;
    }

    async fn get_private_flow(&self, block_num: u64, block_hash: U256) -> HashSet<TxHash> {
        let private_txs = self
            .client
            .query_all_params::<String, String>(
                PRIVATE_FLOW,
                vec![block_num.to_string(), format!("{:#x}", block_hash)],
            )
            .await
            .unwrap();

        private_txs
            .into_iter()
            .map(|tx| TxHash::from_str(&tx).unwrap())
            .collect::<HashSet<TxHash>>()
    }

    async fn get_relay_info(&self, block_num: u64, block_hash: U256) -> RelayInfo {
        self.client
            .query_one_params(
                RELAY_P2P_TIMES,
                vec![block_num.to_string(), format!("{:#x}", block_hash)],
            )
            .await
            .unwrap()
    }

    async fn get_cex_prices(
        &self,
        relay_time: u64,
        p2p_time: u64,
    ) -> HashMap<Address, (Rational, Rational)> {
        let prices = self
            .client
            .query_all_params::<u64, DBTokenPrices>(PRICES, vec![relay_time, p2p_time])
            .await
            .unwrap();

        let token_prices = prices
            .into_iter()
            .map(|row| {
                (
                    row.address,
                    (
                        Rational::try_from(row.price0).unwrap(),
                        Rational::try_from(row.price1).unwrap(),
                    ),
                )
            })
            .collect::<HashMap<Address, (Rational, Rational)>>();

        token_prices
    }
}

#[cfg(test)]
mod tests {

    use dotenv::dotenv;
    use reth_primitives::{H160, H256};

    use super::*;

    const BLOCK_NUMBER: u64 = 18180900;
    const BLOCK_HASH: &str = "0x2c6bb65135fd200b7bb92fc9e63017d26a61a34d8ccdb6f6a501dc73bc32ce41";

    fn expected_private_flow() -> HashSet<H256> {
        let set = vec![
            H256::from_str("0x6343399486888e07485ea91f1096b55f5508767df88d928376094318a346bc81")
                .unwrap(),
            H256::from_str("0x4ca5bf9ace6088f1c58cd5a0bbf5830864c4c344b39046ad288775e7e94e58de")
                .unwrap(),
            H256::from_str("0xd328a2afe816f680c495545373fb2d739829c56c88714e9f34326e73ccfe54f2")
                .unwrap(),
            H256::from_str("0x94fee12653f619bb8c7fcec60eb995d50c7430f497583acb45486362973ab1d3")
                .unwrap(),
            H256::from_str("0xc78bf07b9b7b43058e11bb0da08ca29cc9143374aec3a129bef08007f1204477")
                .unwrap(),
            H256::from_str("0xa8cacf2de6e06b83372bf8e1e5264df28d1b810125e6b124cf7a95dfabaeda16")
                .unwrap(),
            H256::from_str("0x89f1c8ae2820654dd03735802f85114f910b3cb63ec8e7408c5464588097a7b4")
                .unwrap(),
            H256::from_str("0x95b67f167abb03b585c2aedb636843797fd02511449dd614a15a772840a3d039")
                .unwrap(),
            H256::from_str("0xb1e92797d49c003f4ee933dd4529ee63568057243ee0dbc4c3ff9f1eb58d6f95")
                .unwrap(),
            H256::from_str("0x75a9c74e6e50adf5debf2e83254d0da58314eb07eefc2ae25718df66b480bb6f")
                .unwrap(),
            H256::from_str("0xdab644a36ffbcdbba36f4950db373ea7377d3a6204a639561e2a1f9a59cbef3f")
                .unwrap(),
            H256::from_str("0x2d43c26e7e7d01cd5e139e710467fb1045ae5d5a5be4bfbaa4b0dee07bdb5edb")
                .unwrap(),
            H256::from_str("0xd79a2c1b9dd8d55ea05eb3653c572c827951fb40f52ff7562801a8b8cca484e0")
                .unwrap(),
            H256::from_str("0x17eb3b75fbf381ac2ef8840763c93363b962255660568194b8da5f6c7aefdc3c")
                .unwrap(),
            H256::from_str("0xf95dfb7950d6cf3cf4aa342239d823d2e77916baf48100bb961d2dfaea63cd49")
                .unwrap(),
            H256::from_str("0xf5de99a8d45aa1b65138b62ed7f77de75efac2d934b6903a3a3927ae4fa9d252")
                .unwrap(),
            H256::from_str("0xce29f9146afd9e954c0f60b0b05b6ede02b73df6512ff0977fb748c72a1ffebd")
                .unwrap(),
            H256::from_str("0xf4e4b9faf3801718b9f1a95ec39970b1831c6efd3dd57c14cbfd77a1e9595802")
                .unwrap(),
            H256::from_str("0xf1b1ab5fcd494071a41764a3847542dfa94251dd81909ea2525a29038d887d5f")
                .unwrap(),
            H256::from_str("0xbf8c489f02b769046452abae13a6d24d2793f0a098937c2db9f71828f0254b81")
                .unwrap(),
            H256::from_str("0x9f18e09fd33378e75f5266b70201d0a7e63d8e7759c571c0a1fe5b3acc83ed7a")
                .unwrap(),
            H256::from_str("0xf7adf7b12196f424b5845b8a56701b06bc8a7f06ed53aa455e41b05b3a4f4a94")
                .unwrap(),
            H256::from_str("0x18484a968cac9ea2dfb9d357d7e14642f474ebfaa447d4812488ae77af6f61e4")
                .unwrap(),
        ];

        HashSet::from_iter(set.into_iter())
    }

    fn expected_relay_info() -> RelayInfo {
        RelayInfo {
            relay_time:      1695258707776,
            p2p_time:        1695258708673,
            proposer_addr:   H160::from_str("0x388C818CA8B9251b393131C08a736A67ccB19297").unwrap(),
            proposer_reward: 113949354337187568,
        }
    }

    fn expected_metadata(cex_prices: HashMap<Address, (Rational, Rational)>) -> Metadata {
        let mut cex_prices = cex_prices.clone();
        let eth_prices = &cex_prices
            .get(&H160::from_str("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2").unwrap())
            .unwrap()
            .clone();

        cex_prices
            .remove(&H160::from_str("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2").unwrap())
            .unwrap();

        Metadata {
            block_num:              BLOCK_NUMBER,
            block_hash:             H256::from_str(BLOCK_HASH).unwrap().into(),
            relay_timestamp:        1695258707776,
            p2p_timestamp:          1695258708673,
            proposer_fee_recipient: H160::from_str("0x388C818CA8B9251b393131C08a736A67ccB19297")
                .unwrap(),
            proposer_mev_reward:    113949354337187568,
            token_prices:           cex_prices.clone(),
            eth_prices:             eth_prices.clone(),
            mempool_flow:           expected_private_flow(),
        }
    }

    #[tokio::test]
    async fn test_get_private_flow() {
        dotenv().ok();

        let db = Database::default();

        let expected_private_flow = expected_private_flow();
        let private_flow = db
            .get_private_flow(BLOCK_NUMBER, H256::from_str(BLOCK_HASH).unwrap().into())
            .await;

        assert_eq!(expected_private_flow, private_flow)
    }

    #[tokio::test]
    async fn test_get_relay_info() {
        dotenv().ok();

        let db = Database::default();

        let expected_relay_info = expected_relay_info();
        let relay_info = db
            .get_relay_info(BLOCK_NUMBER, H256::from_str(BLOCK_HASH).unwrap().into())
            .await;

        assert_eq!(expected_relay_info.relay_time, relay_info.relay_time);
        assert_eq!(expected_relay_info.p2p_time, relay_info.p2p_time);
        assert_eq!(expected_relay_info.proposer_addr, relay_info.proposer_addr);
        assert_eq!(expected_relay_info.proposer_reward, relay_info.proposer_reward);
    }

    #[tokio::test]
    async fn test_get_cex_prices() {
        dotenv().ok();

        let db = Database::default();

        let cex_prices = db.get_cex_prices(1695258707776, 1695258708673).await;

        let real_prices = cex_prices
            .get(&H160::from_str("5cf04716ba20127f1e2297addcf4b5035000c9eb").unwrap())
            .unwrap()
            .clone();
        let queried_prices =
            (Rational::try_from(0.086237).unwrap(), Rational::try_from(0.086237).unwrap());
        assert_eq!(real_prices, queried_prices);

        let real_prices = cex_prices
            .get(&H160::from_str("c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2").unwrap())
            .unwrap()
            .clone();
        let queried_prices =
            (Rational::try_from(1634.337859).unwrap(), Rational::try_from(1634.337786).unwrap());
        assert_eq!(real_prices, queried_prices);

        assert_eq!(cex_prices.len(), 17);
    }

    #[tokio::test]
    async fn test_get_metadata() {
        dotenv().ok();

        let db = Database::default();

        let cex_prices = db.get_cex_prices(1695258707776, 1695258708673).await;

        let expected_metadata = expected_metadata(cex_prices);

        let metadata = db
            .get_metadata(BLOCK_NUMBER, H256::from_str(BLOCK_HASH).unwrap().into())
            .await;

        assert_eq!(metadata.block_num, expected_metadata.block_num);
        assert_eq!(metadata.block_hash, expected_metadata.block_hash);
        assert_eq!(metadata.relay_timestamp, expected_metadata.relay_timestamp);
        assert_eq!(metadata.p2p_timestamp, expected_metadata.p2p_timestamp);
        assert_eq!(metadata.proposer_fee_recipient, expected_metadata.proposer_fee_recipient);
        assert_eq!(metadata.proposer_mev_reward, expected_metadata.proposer_mev_reward);
        assert_eq!(metadata.mempool_flow, expected_metadata.mempool_flow);
    }
}
