pub mod errors;
pub(crate) mod serialize;
pub mod types;
use malachite::Rational;
use std::collections::{HashMap, HashSet};
use std::str::FromStr;

use super::Metadata;
use crate::const_sql::*;
use crate::database::types::{DBP2PRelayTimes, DBTardisTrades};
use clickhouse::{Client, Row};
use hyper_tls::HttpsConnector;
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
/// NEED TO FIX DESERIALIZATION -- IDK Y THIS IS TWEAKING WILL FIX
/// NEED TO WRITE QUERY FOR ETH PRICE
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
        let private_txs = private_txs
            .into_iter()
            .map(|tx| TxHash::from_str(&tx).unwrap())
            .collect::<HashSet<TxHash>>();

        let times = self
            .client
            .query_one_params::<String, (u64, u64)>(
                RELAYS_P2P_TIME,
                vec![block_num.to_string(), format!("{:#x}", block_hash)],
            )
            .await
            .unwrap();

        let prices = self
            .client
            .query_all_params::<u64, (String, f64, f64)>(
                PRICE,
                vec![times.0, times.0, times.1, times.1],
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

        let metadata = Metadata::new(
            block_num,
            block_hash,
            times.0,
            times.1,
            token_prices,
            Default::default(),
            private_txs,
        );

        metadata
    }
}
