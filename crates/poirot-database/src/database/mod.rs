pub mod const_sql;
pub mod errors;
pub(crate) mod serialize;
pub mod types;
use std::{
    collections::{HashMap, HashSet},
    str::FromStr,
};

use malachite::Rational;
use poirot_types::classified_mev::{ClassifiedMev, MevBlock, SpecificMev};
use reth_primitives::{Address, TxHash, U256};
use serde::Deserialize;
use sorella_db_clients::databases::clickhouse::{self, ClickhouseClient, Row};

use self::types::DBTokenPrices;
use super::Metadata;
use crate::database::const_sql::*;

pub struct Database {
    client: ClickhouseClient,
}

#[derive(Debug, Clone, Row, Deserialize)]
pub struct RelayInfo {
    pub relay_time:      u64,
    pub p2p_time:        u64,
    pub proposer_addr:   Address,
    pub proposer_reward: u64,
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
