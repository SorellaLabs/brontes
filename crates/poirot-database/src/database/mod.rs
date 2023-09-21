pub mod const_sql;
pub mod errors;
pub(crate) mod serialize;
pub mod types;
use std::{
    collections::{HashMap, HashSet},
    str::FromStr,
};

use malachite::Rational;
use poirot_types::classified_mev::{ClassifiedMev, MevBlock, MevResult};
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

/// DO ERROR HANDLING - ERROR TYPE 'DatabaseError'
/// MAKE THIS ASYNC
/// NEED TO FIX DESERIALIZATION -- IDK Y THIS IS TWEAKING WILL FIX
/// NEED TO WRITE QUERY FOR ETH PRICE
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
        mev_details: Vec<(ClassifiedMev, MevResult)>,
    ) {
        todo!()
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
            .query_all_params::<u64, DBTokenPrices>(
                PRICES,
                vec![relay_time, relay_time, p2p_time, p2p_time],
            )
            .await
            .unwrap();

        let token_prices = prices
            .into_iter()
            .map(|row| {
                (
                    Address::from_str(&row.address).unwrap(),
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
    use reth_primitives::H256;

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
}
