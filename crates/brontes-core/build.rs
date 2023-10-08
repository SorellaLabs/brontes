use std::{
    collections::HashSet,
    env,
    fs::{self, File},
    io::{BufWriter, Write},
    path::Path,
    str::FromStr,
};

use clickhouse::{Client, Row};
use ethers_core::types::Chain;
use futures::{future::join_all, FutureExt};
use hyper_tls::HttpsConnector;
use reth_primitives::{Address, BlockId, BlockNumberOrTag, H160};
use reth_rpc_types::trace::parity::TraceType;
use reth_tracing::TracingClient;
use serde::{Deserialize, Serialize};
use serde_json::Value;

const ABI_DIRECTORY: &str = "./abis/";
const PROTOCOL_ADDRESS_SET_PATH: &str = "protocol_addr_set.rs";
const BINDINGS_PATH: &str = "bindings.rs";

const DATA_QUERY: &str = r#"
SELECT
    arrayMap(x -> toString(x), groupArray(ca.address)) AS addresses,
    c.abi AS abi ,
    c.classifier_name AS classifier_name
FROM ethereum.addresses AS ca
LEFT JOIN ethereum.contracts AS c ON ca.hashed_bytecode = c.hashed_bytecode
GROUP BY
    ca.hashed_bytecode,
    c.abi,
    c.classifier_name
HAVING hashed_bytecode != 'NULL' 
"#;

const DATA_QUERY_FILTER: &str = r#"
SELECT
    arrayMap(x -> toString(x), groupArray(ca.address)) AS addresses,
    c.abi AS abi ,
    c.classifier_name AS classifier_name
FROM ethereum.addresses AS ca
LEFT JOIN ethereum.contracts AS c ON ca.hashed_bytecode = c.hashed_bytecode
GROUP BY
    ca.hashed_bytecode,
    c.abi,
    c.classifier_name
HAVING hashed_bytecode != 'NULL' AND hasAny(addresses, ?) OR c.classifier_name != ''
"#;

#[derive(Debug, Serialize, Deserialize, Row, Clone, Default)]
struct ProtocolDetails {
    pub addresses:       Vec<String>,
    pub abi:             String,
    pub classifier_name: String,
}

fn main() {
    dotenv::dotenv().ok();

    println!("cargo:rerun-if-env-changed=RUN_BUILD_SCRIPT");
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    runtime.block_on(run());
}

async fn run() {
    let clickhouse_client = build_db();
    #[cfg(feature = "test_run")]
    let addresses = {
        let start_block = env::var("START_BLOCK").expect("START_BLOCK not found in env");
        let end_block = env::var("END_BLOCK").expect("END_BLOCK not found in env");

        get_all_touched_addresses(
            u64::from_str_radix(&start_block, 10).unwrap(),
            u64::from_str_radix(&end_block, 10).unwrap(),
        )
        .await
        .into_iter()
        .map(|addr| format!("{:?}", addr).to_lowercase())
        .collect::<Vec<_>>()
    };

    #[cfg(feature = "server")]
    let mut protocol_abis = {
        #[cfg(not(feature = "test_run"))]
        {
            query_db::<ProtocolDetails>(&clickhouse_client, DATA_QUERY).await
        }
        #[cfg(feature = "test_run")]
        clickhouse_client
            .query(DATA_QUERY_FILTER)
            .bind(addresses.clone())
            .fetch_all()
            .await
            .unwrap()
    };
    #[cfg(not(feature = "server"))]
    let mut protocol_abis = vec![ProtocolDetails::default()];

    write_all_abis(&protocol_abis);

    generate(
        Path::new(&env::var("OUT_DIR").unwrap())
            .join(BINDINGS_PATH)
            .to_str()
            .unwrap(),
        &protocol_abis,
    )
    .await;
    address_abi_mapping(protocol_abis)
}

#[cfg(feature = "test_run")]
async fn get_all_touched_addresses(start_block: u64, end_block: u64) -> HashSet<Address> {
    let db_path = env::var("DB_PATH").expect("DB_PATH not found in env");
    let tracer = TracingClient::new(Path::new(&db_path), tokio::runtime::Handle::current());

    let mut trace_type = HashSet::new();
    trace_type.insert(TraceType::Trace);
    trace_type.insert(TraceType::VmTrace);

    join_all((start_block..end_block).into_iter().map(|block_num| {
        tracer
            .trace
            .replay_block_transactions(
                BlockId::Number(BlockNumberOrTag::Number(block_num)),
                trace_type.clone(),
            )
            .map(|trace| {
                trace.unwrap().unwrap().into_iter().flat_map(|trace| {
                    trace
                        .full_trace
                        .trace
                        .into_iter()
                        .filter_map(|call_frame| match call_frame.action {
                            reth_rpc_types::trace::parity::Action::Call(c) => Some(c.to),
                            reth_rpc_types::trace::parity::Action::Create(_)
                            | reth_rpc_types::trace::parity::Action::Reward(_) => None,
                            reth_rpc_types::trace::parity::Action::Selfdestruct(s) => {
                                Some(s.address)
                            }
                        })
                        .collect::<Vec<_>>()
                })
            })
    }))
    .await
    .into_iter()
    .flatten()
    .collect::<HashSet<_>>()
}

//
//
// ------------------------ BINDINGS ------------------------
//
//

/// generates all bindings and enums for them and writes them to a file
async fn generate(bindings_file_path: &str, addresses: &Vec<ProtocolDetails>) {
    let mut file = write_file(bindings_file_path, true);

    let mut addr_bindings = Vec::new();
    let mut binding_enums = init_enum("StaticBindings");
    let mut return_binding_enums = init_enum("StaticReturnBindings");
    let mut mod_enums = Vec::new();
    let mut bindings_impl_try_decode = bindings_try_decode_impl_init();

    for protocol_addr in addresses {
        let name = if protocol_addr.classifier_name.is_empty() {
            protocol_addr
                .addresses
                .first()
                .map(|string| "Contract".to_string() + string)
                .unwrap()
        } else {
            protocol_addr.classifier_name.clone()
        };
        let name = &name;

        let abi_file_path = get_file_path(ABI_DIRECTORY, name, ".json");
        addr_bindings.push(binding_string(&abi_file_path, name));

        binding_enums.push(enum_binding_string(name, Some("_Enum")));
        return_binding_enums.push(enum_binding_string(
            &protocol_addr.classifier_name,
            Some(&format!("::{}Calls", name)),
        ));
        individual_sub_enums(&mut mod_enums, name);
        enum_impl_macro(&mut mod_enums, name);
        bindings_impl_try_decode.push(bindings_try_row(name));
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
    bindings.push(format!("#[repr(u32)]\n pub enum {} {{", name));

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
        "       StaticBindings::{}(_) => \
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
fn write_all_abis(protos: &Vec<ProtocolDetails>) {
    for protocol_addr in protos {
        let name = if protocol_addr.classifier_name.is_empty() {
            protocol_addr
                .addresses
                .first()
                .map(|string| "Contract".to_string() + string)
                .unwrap()
        } else {
            protocol_addr.classifier_name.clone()
        };

        let abi_file_path = get_file_path(ABI_DIRECTORY, &name, ".json");
        let mut file = write_file(&abi_file_path, true);
        let decoded: Value = serde_json::from_str(&protocol_addr.abi).unwrap();
        file.write_all(&serde_json::to_vec_pretty(&decoded).unwrap())
            .unwrap();
    }
}

/// creates a mapping of each address to an abi binding
fn address_abi_mapping(mapping: Vec<ProtocolDetails>) {
    let path = Path::new(&env::var("OUT_DIR").unwrap()).join(PROTOCOL_ADDRESS_SET_PATH);
    let mut file = BufWriter::new(File::create(&path).unwrap());

    let mut phf_map = phf_codegen::Map::new();
    for map in mapping {
        if map.classifier_name.is_empty() {
            let name = "Contract".to_string() + map.addresses.first().unwrap();
            for address in map.addresses {
                phf_map.entry(
                    H160::from_str(&address).unwrap().0,
                    &format!("(None, StaticBindings::{}({}_Enum::None))", name, name),
                );
            }
        } else {
            for address in map.addresses {
                let name = &map.classifier_name;
                phf_map.entry(
                    H160::from_str(&address).unwrap().0,
                    &format!(
                        "(Some({}::default()), StaticBindings::{}({}_Enum::None))",
                        name, name, name
                    ),
                );
            }
        }
    }

    writeln!(
        &mut file,
        "pub static PROTOCOL_ADDRESS_MAPPING: phf::Map<[u8; 20], (Option<Box<dyn \
         ActionCollection>>,StaticBindings)> = \n{};\n",
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

/// queries the db
async fn query_db<T: Row + for<'a> Deserialize<'a>>(db: &Client, query: &str) -> Vec<T> {
    db.query("OPTIMIZE TABLE ethereum.pools FINAL DEDUPLICATE BY *")
        .execute()
        .await
        .unwrap();

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
