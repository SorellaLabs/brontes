use std::{
    collections::HashMap,
    env,
    fs::File,
    hash::Hash,
    io::{BufWriter, Write},
    path::Path,
    str::FromStr,
};

use ethers_core::types::Address;
use hyper_tls::HttpsConnector;
use reth_primitives::H160;
use serde::{Deserialize, Serialize};
use sorella_db_databases::clickhouse::{self, Client, Row};
use strum::Display;

const TOKEN_MAPPING_FILE: &str = "token_mapping.rs";
#[allow(dead_code)]
const TOKEN_QUERIES: &str = "SELECT toString(address) AS address, decimals FROM tokens";

fn main() {
    println!("cargo:rerun-if-env-changed=RUN_BUILD_SCRIPT");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    runtime.block_on(async move {
        dotenv::dotenv().ok();
        let path = Path::new(&env::var("ABI_BUILD_DIR").unwrap()).join(TOKEN_MAPPING_FILE);
        let mut file = BufWriter::new(File::create(&path).unwrap());
        build_token_details_map(&mut file).await;
    });
}

#[derive(Debug, Serialize, Deserialize, Clone, Row)]
pub struct TokenDetails {
    address:  String,
    decimals: u8,
}

async fn build_token_details_map(file: &mut BufWriter<File>) {
    #[allow(unused_mut)]
    let mut phf_map: phf_codegen::Map<[u8; 20]> = phf_codegen::Map::new();

    let client = build_db();
    let rows = query_db::<TokenDetails>(&client, TOKEN_QUERIES).await;

    for row in rows {
        phf_map.entry(H160::from_str(&row.address).unwrap().0, row.decimals.to_string().as_str());
    }

    writeln!(
        file,
        "pub static TOKEN_TO_DECIMALS: phf::Map<[u8; 20], u8> = \n{};\n",
        phf_map.build()
    )
    .unwrap();
}

#[derive(
    Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Copy, Serialize, Deserialize, Display,
)]
pub enum Blockchain {
    /// to represent an all query
    Optimism,
    Ethereum,
    Bsc,
    Gnosis,
    Polygon,
    Fantom,
    Klaytn,
    Arbitrum,
    Avalanche,
    Aurora,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TokenList {
    pub tokens: Vec<Token>,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, Clone)]
pub struct Token {
    pub chain_addresses: HashMap<Blockchain, Vec<Address>>,
    /// e.g USDC, USDT, ETH, BTC
    pub global_id:       String,
}

impl Hash for Token {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.global_id.hash(state)
    }
}

/// builds the clickhouse database client

#[allow(dead_code)]
fn build_db() -> Client {
    // clickhouse path
    let clickhouse_path = format!(
        "{}:{}",
        &env::var("CLICKHOUSE_URL").expect("CLICKHOUSE_URL not found in .env"),
        &env::var("CLICKHOUSE_PORT").expect("CLICKHOUSE_PORT not found in .env")
    );

    // builds the https connector
    let https = HttpsConnector::new();
    let https_client = hyper::Client::builder().build::<_, hyper::Body>(https);

    // builds the clickhouse client

    Client::with_http_client(https_client)
        .with_url(clickhouse_path)
        .with_user(env::var("CLICKHOUSE_USER").expect("CLICKHOUSE_USER not found in .env"))
        .with_password(env::var("CLICKHOUSE_PASS").expect("CLICKHOUSE_PASS not found in .env"))
        .with_database(
            env::var("CLICKHOUSE_DATABASE").expect("CLICKHOUSE_DATABASE not found in .env"),
        )
}

#[allow(dead_code)]
async fn query_db<T: Row + for<'a> Deserialize<'a> + Send>(db: &Client, query: &str) -> Vec<T> {
    db.query(query).fetch_all::<T>().await.unwrap()
}
