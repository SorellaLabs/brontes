pub mod errors;
pub(crate) mod serialize;
pub mod types;
use std::collections::{HashMap, HashSet};

use super::Metadata;
use crate::const_sql::*;
use crate::database::types::{DBP2PRelayTimes, DBTardisTrades};
use clickhouse::{Client, Row};
use hyper_tls::HttpsConnector;
use malachite::Rational;
use reth_primitives::{Address, TxHash, U256};
use sorella_db_clients::{databases::clickhouse::ClickhouseClient, errors::DatabaseError};

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
impl Database {
    pub async fn get_metadata(&self, block_num: u64, block_hash: U256) -> Metadata {
        let private_txs = self
            .client
            .query_all_params::<String, String>(
                PRIVATE_FLOW,
                vec![block_num.to_string(), format!("{:#x}", block_hash)],
            )
            .await
            .unwrap();

        let times = self
            .client
            .query_one_params::<String, DBP2PRelayTimes>(
                RELAYS_P2P_TIME,
                vec![block_num.to_string(), format!("{:#x}", block_hash)],
            )
            .await
            .unwrap();

        let prices = self
            .client
            .query_all_params::<u64, DBTardisTrades>(
                PRICE,
                vec![
                    times.relay_timestamp,
                    times.relay_timestamp,
                    times.p2p_timestamp,
                    times.p2p_timestamp,
                ],
            )
            .await
            .unwrap();

        let metadata = 
        todo!()
    }
}
