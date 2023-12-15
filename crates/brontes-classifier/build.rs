use std::{
    collections::HashSet,
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

const PROTOCOLS: &str =
    "select distinct concat(protocol, protocol_subtype) as name from ethereum.pools";

const PROTOCOL_CLASSIFICATION_LOCATION: &str = "protocol_classifier_map.rs";

#[derive(Debug, Serialize, Deserialize, Row, Clone, Default, PartialEq, Eq, Hash)]
struct ProtocolName {
    name: String,
}

fn main() {
    dotenv::dotenv().ok();
    // println!("cargo:rerun-if-env-changed=RUN_BUILD_SCRIPT");
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    runtime.block_on(build_classifier_map());
}

async fn build_classifier_map() {
    let clickhouse_client = build_db();
    let path = Path::new(&env::var("ABI_BUILD_DIR").unwrap())
        .join(PROTOCOL_CLASSIFICATION_LOCATION)
        .to_str()
        .unwrap();

    let mut file = BufWriter::new(File::create(&path).unwrap());
    let names = query_db::<ProtocolName>(&clickhouse_client, PROTOCOLS).await;

    let mut phf_map = phf_codegen::Map::new();
    for name in names {
        let classified_name = name.name.clone() + "Classifier";
        phf_map.entry(name.name.as_str(), &format!("Lazy::new(|| Box::new({}::default()))", classified_name));
    }

    writeln!(
        &mut file,
        "pub static PROTOCOL_CLASSIFIER_MAPPING: phf::Map<&'static str, Lazy<Box<dyn \
         ActionCollection>>> = \n{};\n",
        phf_map.build()
    )
    .unwrap();
}

/// builds the clickhouse database client
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

//
//
// ------------------------ FILE UTILS ------------------------
//
//

/// generates a file path as <DIRECTORY>/<FILENAME><SUFFIX>
fn get_file_path(directory: &str, file_name: &str, suffix: &str) -> String {
    let mut file_path = directory.to_string();
    file_path.push_str(file_name);
    file_path.push_str(suffix);
    file_path
}

/// returns a writeable file
fn write_file(file_path: &str, create: bool) -> File {
    if create {
        File::create(file_path).unwrap();
    }

    fs::OpenOptions::new()
        .write(true)
        .read(true)
        .open(file_path)
        .expect("could not open file")
}

fn parse_filtered_addresses(file: &str) -> HashSet<String> {
    std::fs::read_to_string(file)
        .map(|data| data.split(',').map(|s| s.trim().to_string()).collect())
        .unwrap_or_default()
}

async fn query_db<T: Row + for<'a> Deserialize<'a> + Send>(db: &Client, query: &str) -> Vec<T> {
    db.query(query).fetch_all::<T>().await.unwrap()
}
