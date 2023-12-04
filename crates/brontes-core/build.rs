use std::{
    collections::HashMap,
    env,
    fs::{self, File},
    io::{BufWriter, Write},
    path::Path,
    str::FromStr,
};

use alloy_json_abi::JsonAbi;
use clickhouse::{Client, Row};
use hyper_tls::HttpsConnector;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use reth_primitives::Address;
use serde::{Deserialize, Serialize};
use serde_json::Value;

const POOLS_QUERY: &str = "SELECT protocol, protocol_subtype, toString(address), arrayMap(x -> toString(x), tokens) AS tokens FROM ethereum.pools WHERE length(tokens) = 2"; 

const DEX_PRICE_MAP: &str = "dex_price_map.rs";

#[derive(Debug, Clone, Row, Serialize, Deserialize)]
pub struct Pools {
    protocol: String,
    protocol_subtype: String,
    address: String,
    tokens: Vec<String>,
}

fn main() {
    dotenv::dotenv().ok();
    println!("cargo:rerun-if-env-changed=RUN_BUILD_SCRIPT");
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    runtime.block_on(build_dex_pricing_map());
}


pub async fn build_dex_pricing_map() {
    let path = Path::new(&env::var("ABI_BUILD_DIR").unwrap()).join(DEX_PRICE_MAP);
    let mut file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .open(path)
        .expect("could not open file");

    let client = build_db();
    let data: Vec<Pools> = query_db::<Pools>(&client, POOLS_QUERY).await;

    let mut map: HashMap<[u8;40], Vec<(bool, String, String)>> = HashMap::default();

    data.into_iter().for_each(|pool| {
        let token0 = Address::from_str(&pool.tokens[0]).unwrap().0.0;
        let token1 = Address::from_str(&pool.tokens[1]).unwrap().0.0;
        let protocol = format!("{}{}", pool.protocol, pool.protocol_subtype);


        map.entry(combine_slices(token0, token1)).or_default().push((true, pool.address.clone(), protocol.clone()));
        map.entry(combine_slices(token1, token0)).or_default().push((false, pool.address, protocol));
    });

    let mut phf_map = phf_codegen::Map::new();

    for (k, v) in map {
        phf_map.entry(k, &build_vec_of_details(v));
    }

    writeln!(
        file,
        "pub static DEX_PRICE_MAP: phf::Map<[u8; 40], &[(bool, Address, Lazy<Box<dyn DexPrice>>)]> = \n{};\n",
        phf_map.build()
    )
    .unwrap();

}
fn combine_slices(slice1: [u8; 20], slice2: [u8; 20]) -> [u8; 40] {
    let mut combined = [0u8; 40];

    combined[..20].copy_from_slice(&slice1);
    combined[20..].copy_from_slice(&slice2);

    combined
}

fn build_vec_of_details(values: Vec<(bool, String, String)>) -> String {
    let mut start = "&[".to_string();

    for (zto, address, protocol) in values {
        start += &format!("({zto},");
        let addr = Address::from_str(&address).unwrap();
        start += "Address(FixedBytes(";
        start += &format!("{:?}",addr.0.0);
        start += ")),";
        start += &format!("Lazy::new(|| Box::new({}DexPrice::default()))),", protocol);
    }

    let _ = start.pop();
    start += "]";

    start
}

async fn query_db<T: Row + for<'a> Deserialize<'a> + Send>(db: &Client, query: &str) -> Vec<T> {
    db.query(query).fetch_all::<T>().await.unwrap()

}

fn build_db() -> Client {
    dotenv::dotenv().ok();
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
