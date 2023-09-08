use alloy_json_abi::JsonAbi;
use clickhouse::{Client, Row};
use ethers_core::types::{Chain, H160};
use hyper_tls::HttpsConnector;
use phf_codegen::Map;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    env,
    fs::{self, File},
    io::{BufReader, BufWriter, Write},
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

#[derive(Debug, Serialize, Deserialize, Row, Clone)]
struct ProtocolAbis {
    protocol: String,
    address: String,
}

fn main() {
    dotenv::dotenv().ok();
    println!("cargo:rerun-if-env-changed=RUN_BUILD_SCRIPT");
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();

    runtime.block_on(run());
}

async fn run() {
    let clickhouse_client = build_db();
    let etherscan_client = build_etherscan();

    let protocol_abis = query_db::<ProtocolAbis>(&clickhouse_client, PROTOCOL_ABIS).await;

    let abis = write_all_abis(etherscan_client, protocol_abis.clone()).await;

    let protocol_address_map =
        query_db::<AddressToProtocolMapping>(&clickhouse_client, PROTOCOL_ADDRESSES).await;

    generate("./src/bindings.rs", protocol_abis.clone(), abis).await;
    address_abi_mapping(protocol_address_map)
}

//
//
// ------------------------ BINDINGS ------------------------
//
//

/// generates all bindings and enums for them and writes them to a file
async fn generate(bindings_file_path: &str, addresses: Vec<ProtocolAbis>, abis: Vec<Value>) {
    let mut file = write_file(bindings_file_path, true);

    let mut bindings = init_bindings();
    let mut binding_enums = init_enum("StaticBindings");
    let mut mod_enums = Vec::new();

    for (protocol_addr, abi) in addresses.into_iter().zip(abis) {
        let abi_file_path = get_file_path(ABI_DIRECTORY, &protocol_addr.protocol, ".json");
        bindings.push(binding_string(&abi_file_path, &protocol_addr.protocol));

        binding_enums.push(enum_binding_string(&protocol_addr.protocol));

        individual_sub_enums(&mut mod_enums, &abi_file_path, &protocol_addr.protocol);
    }

    binding_enums.push("}".to_string());

    let aggr = format!(
        "{}\n\n{}\n{}",
        bindings.join("\n"),
        binding_enums.join("\n"),
        mod_enums.join("\n")
    );

    file.write_all(aggr.as_bytes()).unwrap();
}

/// generates the bindings for the given abis
fn init_bindings() -> Vec<String> {
    let mut bindings = Vec::new();
    bindings.push("use alloy_sol_types::sol;\n\n".to_string());
    bindings
}

/// generates the bindings for the given abis
fn init_enum(name: &str) -> Vec<String> {
    let mut bindings = Vec::new();
    bindings.push("\n#[allow(non_camel_case_types)]".to_string());
    bindings.push(format!("pub enum {} {{", name));

    bindings
}

/// generates the string of an individual binding
fn binding_string(file_path: &str, protocol_name: &str) -> String {
    let binding = format!("sol! ({}, \"{}\");", protocol_name, file_path);
    //let enum_binding = format!("   {},\n", protocol_name);
    //(binding, enum_binding)
    binding
}

/// generates the string of an enum for a binding
fn enum_binding_string(protocol_name: &str) -> String {
    let binding = format!("   {},", protocol_name);
    binding
}

/// generates the mapping of function selector to decodable type
fn function_selector_mapping(map: &mut Map<[u8; 4]>, abi_file_path: &str, protocol_name: &str) {
    //let abi_file_path = get_file_path(ABI_DIRECTORY, &protocol_addr.protocol, ".json");
    let reader = BufReader::new(File::open(abi_file_path).unwrap());
    let json_abi: JsonAbi = serde_json::from_reader(reader).unwrap();

    for functions in json_abi.functions.values() {
        for function in functions {
            let val =
                format!("StaticBindings::{}({}::{})", protocol_name, protocol_name, function.name);
            let map = map.entry(function.selector(), &val);
        }
    }
}

/// generates the mapping of function selector to decodable type
fn individual_sub_enums(mod_enum: &mut Vec<String>, abi_file_path: &str, protocol_name: &str) {
    let input = fs::read_to_string(abi_file_path).unwrap();
    let json_abi: JsonAbi = serde_json::from_str(&input).unwrap();

    let mut enum_protocol_name = protocol_name.to_string();
    enum_protocol_name.push_str("_Enum");
    mod_enum.extend(init_enum(&enum_protocol_name));
    for functions in json_abi.functions.values() {
        if functions.len() > 1 {
            for (idx, function) in functions.into_iter().enumerate() {
                let val = format!(
                    "   {}_{}({}::{}_{}Call),",
                    &function.name, idx, protocol_name, &function.name, idx
                );
                mod_enum.push(val)
            }
        } else {
            let val = format!(
                "   {}({}::{}Call),",
                &functions[0].name, protocol_name, &functions[0].name
            );
            mod_enum.push(val);
        }
    }

    mod_enum.push("}".to_string());
}

/// generates the matching of function selector to type to decode
fn fn_sig_protocol_mapping() {}

//
//
// ------------------------ ABIs ------------------------
//
//

/// writes the provider json abis to files given the protocol name
async fn write_all_abis(
    client: alloy_etherscan::Client,
    addresses: Vec<ProtocolAbis>,
) -> Vec<Value> {
    let mut abis = Vec::new();
    for protocol_addr in addresses {
        let abi = get_abi(client.clone(), &protocol_addr.address).await;
        abis.push(abi.clone());
        let abi_file_path = get_file_path(ABI_DIRECTORY, &protocol_addr.protocol, ".json");
        let mut file = write_file(&abi_file_path, true);
        file.write_all(serde_json::to_string(&abi).unwrap().as_bytes()).unwrap();
    }

    abis
}

/// creates a mapping of each address to an abi binding
fn address_abi_mapping(mapping: Vec<AddressToProtocolMapping>) {
    let path = Path::new(&env::var("OUT_DIR").unwrap()).join(PROTOCOL_ADDRESS_MAPPING_PATH);
    let mut file = BufWriter::new(File::create(&path).unwrap());
    file.write_all("use crate::bindings::StaticBindings;\n\n".as_bytes()).unwrap();

    let mut phf_map = phf_codegen::Map::new();
    for map in &mapping {
        for address in &map.addresses {
            phf_map.entry(address, &format!("StaticBindings::{}", &map.protocol));
        }
    }

    writeln!(
        &mut file,
        "pub static PROTOCOL_ADDRESS_MAPPING: phf::Map<&'static str, StaticBindings> = \n{};\n",
        phf_map.build()
    )
    .unwrap();
}

//
//
// ------------------------ ETHERSCAN/DATABASE ------------------------
//
//

/// gets the abis (as a serde 'Value') for the given addresses from etherscan
async fn get_abi(client: alloy_etherscan::Client, address: &str) -> Value {
    let raw = client.raw_contract(H160::from_str(&address).unwrap()).await.unwrap();
    serde_json::from_str(&raw).unwrap()
}

/// queries the db
async fn query_db<T: Row + for<'a> Deserialize<'a>>(db: &Client, query: &str) -> Vec<T> {
    db.query(query).fetch_all::<T>().await.unwrap()
}

//
//
// ------------------------ BUILDERS ------------------------
//
//

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
        File::create(&file_path).unwrap();
    }

    fs::OpenOptions::new().append(true).read(true).open(&file_path).expect("could not open file")
}
