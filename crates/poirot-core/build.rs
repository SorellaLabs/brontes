use clickhouse::{Client, Row};
use ethers_core::types::{Chain, H160};
use hyper_tls::HttpsConnector;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    env,
    fs::{self, File},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    str::FromStr,
};

const BINDINGS_DIRECTORY: &str = "./src/";
const ABI_DIRECTORY: &str = "./abis/";
const PROTOCOL_ADDRESS_MAPPING_PATH: &str = "protocol_addr_mapping.rs";
const CACHE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10_000);
const CACHE_DIRECTORY: &str = "../../abi_cache";
const PROTOCOL_ADDRESSES: &str =
    "SELECT protocol, groupArray(toString(address)) AS addresses FROM pools GROUP BY protocol";
const PROTOCOL_ABIS: &str =
    "SELECT protocol, toString(any(address)) AS address FROM pools GROUP BY protocol";

#[derive(Debug, Serialize, Deserialize, Row)]
struct AddressToProtocolMapping {
    protocol: String,
    addresses: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Row)]
struct ProtocolAbis {
    protocol: String,
    address: String,
}

fn main() {
    println!("cargo:rerun-if-env-changed=RUN_BUILD_SCRIPT");
    dotenv::dotenv().ok();
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();

    runtime.block_on(run());
}

async fn run() {
    let clickhouse_client = build_db();
    let etherscan_client = build_etherscan();

    let protocol_abis = query_db::<ProtocolAbis>(&clickhouse_client, PROTOCOL_ABIS).await;

    write_all_abis(etherscan_client, protocol_abis).await;

    let protocol_address_map =
        query_db::<AddressToProtocolMapping>(&clickhouse_client, PROTOCOL_ADDRESSES).await;

    address_abi_mapping(protocol_address_map)
}

/// builds the clickhouse database client
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
    let client = Client::with_http_client(https_client)
        .with_url(clickhouse_path)
        .with_user(env::var("CLICKHOUSE_USER").expect("CLICKHOUSE_USER not found in .env"))
        .with_password(env::var("CLICKHOUSE_PASS").expect("CLICKHOUSE_PASS not found in .env"))
        .with_database(
            env::var("CLICKHOUSE_DATABASE").expect("CLICKHOUSE_DATABASE not found in .env"),
        );
    client
}

/// builds the etherscan client
fn build_etherscan() -> alloy_etherscan::Client {
    alloy_etherscan::Client::new_cached(
        Chain::Mainnet,
        env::var("ETHERSCAN_API_KEY").expect("ETHERSCAN_API_KEY not found in .env"),
        Some(PathBuf::from(CACHE_DIRECTORY)),
        CACHE_TIMEOUT,
    )
    .unwrap()
}

/// queries the db
async fn query_db<T: Row + for<'a> Deserialize<'a>>(db: &Client, query: &str) -> Vec<T> {
    db.query(query).fetch_all::<T>().await.unwrap()
}

/// gets the abi's for the given addresses from etherscan
async fn get_abi(client: alloy_etherscan::Client, address: &str) -> Value {
    let raw = client.raw_contract(H160::from_str(&address).unwrap()).await.unwrap();
    serde_json::from_str(&raw).unwrap()
}

/// writes json abi to file
fn write_file(file_path: &str) -> File {
    File::create(&file_path).unwrap();

    let file = fs::OpenOptions::new()
        .append(true)
        .read(true)
        .open(&file_path)
        .expect("could not open file");

    file
}

/// writes the provider json abis to files given the protocol name
async fn write_all_abis(client: alloy_etherscan::Client, addresses: Vec<ProtocolAbis>) {
    let mut bindings = Vec::new();
    bindings.push("use alloy_sol_types::sol;\n\n".to_string());
    for protocol_addr in addresses {
        let abi = get_abi(client.clone(), &protocol_addr.address).await;
        let abi_file_path = get_file_path(ABI_DIRECTORY, &protocol_addr.protocol, ".json");
        let mut file = write_file(&abi_file_path);
        file.write_all(serde_json::to_string(&abi).unwrap().as_bytes()).unwrap();

        let abi_file_path = get_file_path("./abis/", &protocol_addr.protocol, ".json");
        bindings.push(generate_bindings(&abi_file_path, &protocol_addr.protocol))
    }

    let bindings_file_path = get_file_path(BINDINGS_DIRECTORY, "bindings", ".rs");
    let mut file = write_file(&bindings_file_path);
    let bindings_str = bindings.join("\n");
    file.write_all(bindings_str.as_bytes()).unwrap();
}

/// creates a mapping of each address to an abi
fn address_abi_mapping(mapping: Vec<AddressToProtocolMapping>) {
    let path = Path::new(&env::var("OUT_DIR").unwrap()).join(PROTOCOL_ADDRESS_MAPPING_PATH);
    let mut file = BufWriter::new(File::create(&path).unwrap());

    let mut phf_map = phf_codegen::Map::new();
    for map in &mapping {
        for address in &map.addresses {
            phf_map.entry(address, &format!("\"{}\"", &map.protocol));
        }
    }

    writeln!(
        &mut file,
        "pub static PROTOCOL_ADDRESS_MAPPING: phf::Map<&'static str, &'static str> = \n{};\n",
        phf_map.build()
    )
    .unwrap();
}

/// generates the bindings
fn generate_bindings(file_path: &str, protocol_name: &str) -> String {
    format!("sol! ({}, \"{}\");", protocol_name, file_path)
}

/// generates a file path as <DIRECTORY>/<FILENAME><SUFFIX>
fn get_file_path(directory: &str, file_name: &str, suffix: &str) -> String {
    let mut file_path = directory.to_string();
    file_path.push_str(file_name);
    file_path.push_str(suffix);
    file_path
}
