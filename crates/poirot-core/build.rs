use std::{
    env,
    fs::{self, File},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    str::FromStr,
};

use clickhouse::{Client, Row};
use ethers_core::types::{Chain, H160};
use hyper_tls::HttpsConnector;
use serde::{Deserialize, Serialize};
use serde_json::Value;

const ABI_DIRECTORY: &str = "./abis/";
const PROTOCOL_ADDRESS_SET_PATH: &str = "protocol_addr_set.rs";
const BINDINGS_PATH: &str = "bindings.rs";
const CACHE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10_000);
const CACHE_DIRECTORY: &str = "../../abi_cache";
const PROTOCOL_ADDRESSES: &str =
    "SELECT protocol, groupArray(toString(address)) AS addresses FROM pools GROUP BY protocol";
const PROTOCOL_ABIS: &str =
    "SELECT protocol, toString(any(address)) AS address FROM pools GROUP BY protocol";

#[derive(Debug, Serialize, Deserialize, Row)]
struct AddressToProtocolMapping {
    protocol:  String,
    addresses: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Row, Clone)]
struct ProtocolAbis {
    protocol: String,
    address:  String,
}

fn main() {
    dotenv::dotenv().ok();
    println!("cargo:rerun-if-env-changed=RUN_BUILD_SCRIPT");
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    runtime.block_on(run());
}

async fn run() {
    let clickhouse_client = build_db();
    let etherscan_client = build_etherscan();

    let protocol_abis = query_db::<ProtocolAbis>(&clickhouse_client, PROTOCOL_ABIS).await;

    write_all_abis(etherscan_client, protocol_abis.clone()).await;

    let protocol_address_map =
        query_db::<AddressToProtocolMapping>(&clickhouse_client, PROTOCOL_ADDRESSES).await;

    generate(
        Path::new(&env::var("OUT_DIR").unwrap())
            .join(BINDINGS_PATH)
            .to_str()
            .unwrap(),
        protocol_abis.clone(),
    )
    .await;
    address_abi_mapping(protocol_address_map)
}

//
//
// ------------------------ BINDINGS ------------------------
//
//

/// generates all bindings and enums for them and writes them to a file
async fn generate(bindings_file_path: &str, addresses: Vec<ProtocolAbis>) {
    let mut file = write_file(bindings_file_path, true);

    let mut addr_bindings = Vec::new();
    let mut binding_enums = init_enum("StaticBindings");
    let mut return_binding_enums = init_enum("StaticReturnBindings");
    let mut mod_enums = Vec::new();
    let mut bindings_impl_try_decode = bindings_try_decode_impl_init();

    for protocol_addr in addresses {
        let abi_file_path = get_file_path(ABI_DIRECTORY, &protocol_addr.protocol, ".json");
        addr_bindings.push(binding_string(&abi_file_path, &protocol_addr.protocol));

        binding_enums.push(enum_binding_string(&protocol_addr.protocol, Some("_Enum")));
        return_binding_enums.push(enum_binding_string(
            &protocol_addr.protocol,
            Some(&format!("::{}Calls", &protocol_addr.protocol)),
        ));
        individual_sub_enums(&mut mod_enums, &protocol_addr.protocol);
        enum_impl_macro(&mut mod_enums, &protocol_addr.protocol);
        bindings_impl_try_decode.push(bindings_try_row(&protocol_addr.protocol));
    }

    binding_enums.push("}".to_string());
    return_binding_enums.push("}".to_string());
    bindings_impl_try_decode.push("     }".to_string());
    bindings_impl_try_decode.push(" }".to_string());
    bindings_impl_try_decode.push("}".to_string());

    let aggr = format!(
        "{}\n\n{}\n{}\n\n{}\n{}",
        addr_bindings.join("\n"),
        binding_enums.join("\n"),
        bindings_impl_try_decode.join("\n"),
        return_binding_enums.join("\n"),
        mod_enums.join("\n")
    );

    file.write_all(aggr.as_bytes()).unwrap();
}

/// generates the bindings for the given abis
fn init_enum(name: &str) -> Vec<String> {
    let mut bindings = Vec::new();
    bindings.push("\n#[allow(non_camel_case_types)]".to_string());
    bindings.push(format!("#[repr(u16)]\n pub enum {} {{", name));

    bindings
}

/// generates the string of an individual binding
fn binding_string(file_path: &str, protocol_name: &str) -> String {
    let binding = format!("sol! ({}, \"{}\");", protocol_name, file_path);
    binding
}

/// generates the string of an enum for a binding
fn enum_binding_string(protocol_name: &str, other: Option<&str>) -> String {
    let binding = format!("   {}({}{}),", protocol_name, protocol_name, other.unwrap_or(""));
    binding
}

/// generates the mapping of function selector to decodable type
fn individual_sub_enums(mod_enum: &mut Vec<String>, protocol_name: &str) {
    let mut enum_protocol_name = protocol_name.to_string();
    enum_protocol_name.push_str("_Enum");
    mod_enum.extend(init_enum(&enum_protocol_name));
    mod_enum.push(" None".to_string());
    mod_enum.push("}".to_string());
}

/// generates the mapping of function selector to decodable type
fn enum_impl_macro(mod_enum: &mut Vec<String>, protocol_name: &str) {
    let macro_impl = format!(
        "impl_decode_sol!({}_Enum, {}::{}Calls);",
        protocol_name, protocol_name, protocol_name
    );
    mod_enum.push(macro_impl);
    mod_enum.push("\n".to_string());
}

/// implements try_decode() for the StaticBindings Enum
fn bindings_try_decode_impl_init() -> Vec<String> {
    let mut impl_str = Vec::new();
    impl_str.push("impl StaticBindings {".to_string());
    impl_str.push(
        " pub fn try_decode(&self, call_data: &[u8]) -> Result<StaticReturnBindings, \
         alloy_sol_types::Error> {"
            .to_string(),
    );
    impl_str.push("     match self {".to_string());
    impl_str
}

/// implements try_decode() for the StaticBindings Enum
fn bindings_try_row(protocol_name: &str) -> String {
    format!(
        "       StaticBindings::{}(val) => \
         Ok(StaticReturnBindings::{}({}_Enum::try_decode(call_data)?)),",
        protocol_name, protocol_name, protocol_name
    )
}

//
//
// ------------------------ ABIs ------------------------
//
//

/// writes the provider json abis to files given the protocol name
async fn write_all_abis(client: alloy_etherscan::Client, addresses: Vec<ProtocolAbis>) {
    for protocol_addr in addresses {
        let abi = get_abi(client.clone(), &protocol_addr.address).await;
        let abi_file_path = get_file_path(ABI_DIRECTORY, &protocol_addr.protocol, ".json");
        let mut file = write_file(&abi_file_path, true);
        file.write_all(serde_json::to_string(&abi).unwrap().as_bytes())
            .unwrap();
    }
}

/// creates a mapping of each address to an abi binding
fn address_abi_mapping(mapping: Vec<AddressToProtocolMapping>) {
    let path = Path::new(&env::var("OUT_DIR").unwrap()).join(PROTOCOL_ADDRESS_SET_PATH);
    let mut file = BufWriter::new(File::create(&path).unwrap());
    //file.write_all("use crate::bindings::*;\n\n".as_bytes()).unwrap();

    let mut phf_map = phf_codegen::Map::new();
    for map in &mapping {
        for address in &map.addresses {
            phf_map.entry(
                address,
                &format!("StaticBindings::{}({}_Enum::None)", &map.protocol, &map.protocol),
            );
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

// TODO! Implement these classifiers for the different protocols:
// 1. UniswapV3
// 2. UniswapV2
// 3. Aave
// 4. Curve
// 5. Compound
// 6. Sushiswap
// 7. Balancer
// 8. Yearn
// 9. Synthetix
// 10. Maker
// 11. 0x
// 12. Bancor
// 13. Kyber
// 14. dYdX
// 15. Ambient

/// gets the abis (as a serde 'Value') for the given addresses from etherscan
async fn get_abi(client: alloy_etherscan::Client, address: &str) -> Value {
    let raw = client
        .raw_contract(H160::from_str(address).unwrap())
        .await
        .unwrap();
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

    Client::with_http_client(https_client)
        .with_url(clickhouse_path)
        .with_user(env::var("CLICKHOUSE_USER").expect("CLICKHOUSE_USER not found in .env"))
        .with_password(env::var("CLICKHOUSE_PASS").expect("CLICKHOUSE_PASS not found in .env"))
        .with_database(
            env::var("CLICKHOUSE_DATABASE").expect("CLICKHOUSE_DATABASE not found in .env"),
        )
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
        File::create(file_path).unwrap();
    }

    fs::OpenOptions::new()
        .append(true)
        .read(true)
        .open(file_path)
        .expect("could not open file")
}
