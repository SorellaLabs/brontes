pub mod errors;
pub(crate) mod serialize;
pub mod types;
use std::collections::{HashMap, HashSet};

use clickhouse::{Client, Row};
use hyper_tls::HttpsConnector;
use malachite::Rational;
use reth_primitives::{Address, TxHash, U256};
use sorella_db_clients::{databases::clickhouse::ClickhouseClient, errors::DatabaseError};

use super::Metadata;

const RELAYS_TABLE: &str = "relays";
const MEMPOOL_TABLE: &str = "chainbound_mempool";
const TARDIS_QUOTES_L2: &str = "tardis_l2";
const TARDIS_QUOTES_QUOTES: &str = "tardis_quotes";
const TARDIS_QUOTES_TRADES: &str = "tardis_trades";
use std::env;

pub struct Database {
    client: ClickhouseClient
}

impl Default for Database {
    fn default() -> Self {
        Self { client: ClickhouseClient::default() }
    }
}

impl Database {
    pub async fn get_metadata(&self, block_num: u64, block_hash: U256) -> Metadata {
        let query = format!("SELECT * FROM {} LIMIT 1", RELAYS_TABLE);
        //let res = self.client.query_all::<types::Relay>(&query).await?;
        //println!("{:?}", res);

        todo!()
    }
}
