pub mod errors;
pub(crate) mod serialize;
pub mod types;
use sorella_db_clients::databases::clickhouse::ClickhouseClient;
use sorella_db_clients::errors::DatabaseError;
use hyper_tls::HttpsConnector;
use clickhouse::{Client, Row};
use reth_primitives::{U256, Address, TxHash};
use std::collections::{HashMap, HashSet};
use malachite::Rational;


const RELAYS_TABLE: &str = "relays";
const MEMPOOL_TABLE: &str = "chainbound_mempool";
const TARDIS_QUOTES_L2: &str = "tardis_l2";
const TARDIS_QUOTES_QUOTES: &str = "tardis_quotes";
const TARDIS_QUOTES_TRADES: &str = "tardis_trades";
use std::env;

pub struct Database {
    client: ClickhouseClient,
}

impl Default for Database {
    fn default() -> Self {
        Self { client: ClickhouseClient::default() }
    }
}


pub struct Metadata {
    pub block_num: u64,
    pub block_hash: U256,
    pub relay_timestamp: u64,
    pub p2p_timestamp: u64,
    pub prices: HashMap<Address, (Rational, Rational)>,
    pub mempool: HashSet<TxHash>,
}


impl Database {

    pub async fn get_metadata(&self, block_num: u64, block_hash: U256) -> Result<Metadata, DatabaseError>  {
        let query = format!("SELECT * FROM {} LIMIT 1", RELAYS_TABLE);
        //let res = self.client.query_all::<types::Relay>(&query).await?;
        //println!("{:?}", res);

        todo!()
    }


    
}

