pub mod const_sql;
pub mod errors;
pub(crate) mod serialize;
pub mod types;
use std::{
    collections::{HashMap, HashSet},
    str::FromStr,
};

use clickhouse::{Client, Row};
use hyper_tls::HttpsConnector;
use malachite::{vecs::exhaustive::LexFixedLengthVecsFromSingle, Rational};
use reth_primitives::{Address, TxHash, U256};
use sorella_db_clients::{databases::clickhouse::ClickhouseClient, errors::DatabaseError};

use super::Metadata;
use crate::database::{
    const_sql::*,
    types::{DBP2PRelayTimes, DBTardisTrades},
};

const RELAYS_TABLE: &str = "relays";
const MEMPOOL_TABLE: &str = "chainbound_mempool";
const TARDIS_QUOTES_L2: &str = "tardis_l2";
const TARDIS_QUOTES_QUOTES: &str = "tardis_quotes";
const TARDIS_QUOTES_TRADES: &str = "tardis_trades";

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
        let cex_prices = self.get_cex_prices(relay_data.0, relay_data.1).await;

        // eth price is in cex_prices
        let eth_prices = Default::default();

        let metadata = Metadata::new(
            block_num,
            block_hash,
            relay_data.0,
            relay_data.1,
            relay_data.2,
            relay_data.3,
            cex_prices,
            eth_prices,
            private_flow,
        );

        metadata
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

    async fn get_relay_info(&self, block_num: u64, block_hash: U256) -> (u64, u64, Address, u64) {
        let times: (u64, u64, Address, u64) = self
            .client
            .query_one_params(
                RELAY_P2P_TIMES,
                vec![block_num.to_string(), format!("{:#x}", block_hash)],
            )
            .await
            .unwrap();
        times
    }

    async fn get_cex_prices(
        &self,
        relay_time: u64,
        p2p_time: u64,
    ) -> HashMap<Address, (Rational, Rational)> {
        let prices = self
            .client
            .query_all_params::<u64, (String, f64, f64)>(
                PRICES,
                vec![relay_time, relay_time, p2p_time, p2p_time],
            )
            .await
            .unwrap();

        let token_prices = prices
            .into_iter()
            .map(|row| {
                (
                    Address::from_str(&row.0).unwrap(),
                    (Rational::try_from(row.1).unwrap(), Rational::try_from(row.2).unwrap()),
                )
            })
            .collect::<HashMap<Address, (Rational, Rational)>>();

        token_prices
    }
}
