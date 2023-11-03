//#![feature(result_option_inspect)]
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
#[cfg(feature = "server")]
use ethers_core::types::Chain;
#[cfg(feature = "server")]
use futures::{future::join_all, FutureExt};
use hyper_tls::HttpsConnector;
use itertools::Itertools;
#[cfg(feature = "server")]
use rayon::iter::IntoParallelRefIterator;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use reth_primitives::{Address, H160};
#[cfg(feature = "server")]
use reth_primitives::{BlockId, BlockNumberOrTag};
use reth_rpc_types::trace::parity::TraceResultsWithTransactionHash;
#[cfg(feature = "server")]
use reth_rpc_types::trace::parity::TraceType;
#[cfg(feature = "server")]
use reth_tracing::TracingClient;
use serde::{Deserialize, Serialize};
use serde_json::Value;

const TOKEN_MAPPING: &str = "token_to_addresses.rs";
const TOKEN_QUERIES: &str = "SELECT toString(address), arrayMap(x -> toString(x), tokens) AS 
                             tokens FROM pools WHERE length(tokens) = ";

const FAILED_ABI_FILE: &str = "../../failed_abis.txt";
const ABI_DIRECTORY: &str = "./abis/";
const PROTOCOL_ADDRESS_SET_PATH: &str = "protocol_addr_set.rs";
const BINDINGS_PATH: &str = "bindings.rs";

/*
const DATA_QUERY: &str = r#"
SELECT arrayMap(x -> toString(x), groupArray(a.address)) as addresses, c.abi, c.classifier_name
FROM ethereum.addresses AS a
INNER JOIN ethereum.contracts AS c ON a.hashed_bytecode = c.hashed_bytecode WHERE a.hashed_bytecode != 'NULL'
GROUP BY c.abi, c.classifier_name
HAVING abi IS NOT NULL
"#;
same as below
*/

const DATA_QUERY: &str = r#"
SELECT 
	groupArray(addresses) as addresses, abi, classifier_name
FROM brontes.protocol_details
GROUP BY abi, classifier_name
"#;

/*
const DATA_QUERY_FILTER: &str = r#"
SELECT arrayMap(x -> toString(x), groupArray(a.address)) as addresses, c.abi, c.classifier_name
FROM ethereum.addresses AS a
INNER JOIN ethereum.contracts AS c ON a.hashed_bytecode = c.hashed_bytecode WHERE a.hashed_bytecode != 'NULL'
GROUP BY c.abi, c.classifier_name
HAVING (abi IS NOT NULL AND hasAny(addresses, ?)) OR classifier_name != ''
"#;
same as below
*/

const CLASSIFIED_ONLY_DATA_QUERY: &str = r#"
SELECT 
	groupArray(addresses) as addresses, abi, classifier_name
FROM brontes.protocol_details
WHERE classifier_name IS NOT NULL
GROUP BY abi, classifier_name
"#;

#[derive(Debug, Serialize, Deserialize, Row, Clone, Default, PartialEq, Eq, Hash)]
struct ProtocolDetails {
    pub addresses: Vec<String>,
    pub abi: Option<String>,
    pub classifier_name: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Row)]
pub struct DecodedTokens {
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

    runtime.block_on(build_address_to_token_map());
    runtime.block_on(run_classifier_mapping());
}

async fn build_address_to_token_map() {
    let path = Path::new(&env::var("ABI_BUILD_DIR").unwrap()).join(TOKEN_MAPPING);
    let mut file = BufWriter::new(File::create(&path).unwrap());

    {
        let client = build_db();
        for i in 2..4 {
            let res =
                query_db::<DecodedTokens>(&client, &(TOKEN_QUERIES.to_string() + &i.to_string()))
                    .await;

            build_token_map(i, res, &mut file)
        }
    }
}

fn build_token_map(amount: i32, rows: Vec<DecodedTokens>, file: &mut BufWriter<File>) {
    let mut phf_map = phf_codegen::Map::new();

    for row in rows {
        phf_map.entry(
            H160::from_str(&row.address).unwrap().to_fixed_bytes(),
            &to_string_vec(row.tokens),
        );
    }

    writeln!(
        file,
        "pub static ADDRESS_TO_TOKENS_{}_POOL: phf::Map<[u8; 20], [H160; {}]> = \n{};\n",
        amount,
        amount,
        phf_map.build()
    )
    .unwrap();
}

async fn run_classifier_mapping() {
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
        .collect::<HashSet<_>>()
    };

    #[cfg(feature = "server")]
    let mut protocol_abis: Vec<ProtocolDetails> = {
        #[cfg(not(feature = "test_run"))]
        {
            query_db::<ProtocolDetails>(&clickhouse_client, DATA_QUERY).await
        }
        #[cfg(feature = "test_run")]
        {
            clickhouse_client
                .query(CLASSIFIED_ONLY_DATA_QUERY)
                .fetch_all::<ProtocolDetails>()
                .await
                .unwrap()
        }
    };
    #[cfg(not(feature = "server"))]
    let mut protocol_abis = query_db::<ProtocolDetails>(&clickhouse_client, CLASSIFIED_ONLY_DATA_QUERY).await;
    // write_test(protocol_abis.clone());
    let failed_abi_addresses = parse_filtered_addresses(FAILED_ABI_FILE);

    #[cfg(feature = "test_run")]
    {
        let file_path = "/home/shared/brontes/class.txt";
        File::create(file_path);

        let mut file = fs::OpenOptions::new()
            .write(true)
            .read(true)
            .open(file_path)
            .expect("could not open file");

        let str = serde_json::to_string(&protocol_abis.clone()).unwrap();
        file.write_all(str.as_bytes()).unwrap();
    }

    // write_test(failed_abi_addresses.clone());
    // write_test('\n');

    let protocol_abis: Vec<(ProtocolDetails, bool, bool)> = protocol_abis
        .into_par_iter()
        .filter(|contract: &ProtocolDetails| {
            let addrs: HashSet<String> = contract.addresses.clone().into_iter().collect();
            // write_test(addrs.clone());
            // write_test(contract.abi.is_some());
            // write_test(!failed_abi_addresses.is_subset(&addrs));
            contract.abi.is_some()
                && (!failed_abi_addresses.is_subset(&addrs) || failed_abi_addresses.is_empty())
        })
        .filter_map(|contract: ProtocolDetails| {
            match JsonAbi::from_json_str(contract.abi.as_ref().unwrap()) {
                Ok(c) => Some((c, contract)),
                Err(e) => {
                    println!("{:?}, {:#?}", e, contract.addresses);
                    None
                }
            }
        })
        .map(|(abi, contract)| (contract, !abi.functions.is_empty(), !abi.events.is_empty()))
        .collect::<Vec<_>>();

    write_all_abis(&protocol_abis);

    generate(
        Path::new(&env::var("ABI_BUILD_DIR").unwrap())
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

    let range = (start_block..end_block).into_iter().collect::<Vec<_>>();
    let mut res = HashSet::new();

    for chunk in range.as_slice().chunks(5) {
        res.extend(get_addresses_for_chunk(&tracer, chunk).await);
        res.shrink_to_fit();
    }

    res
}

#[cfg(feature = "test_run")]
async fn get_addresses_for_chunk(client: &TracingClient, chunk: &[u64]) -> HashSet<Address> {
    let mut trace_type = HashSet::new();
    trace_type.insert(TraceType::Trace);
    trace_type.insert(TraceType::VmTrace);

    let res = join_all(chunk.into_iter().map(|block_num| {
        client
            .trace
            .replay_block_transactions(
                BlockId::Number(BlockNumberOrTag::Number(*block_num)),
                trace_type.clone(),
            )
            .map(|trace| expand_trace(trace.unwrap().unwrap()))
    }))
    .await
    .into_iter()
    .flatten()
    .collect::<HashSet<_>>();
    println!("got addresses for chunk");

    res
}

fn expand_trace(trace: Vec<TraceResultsWithTransactionHash>) -> HashSet<Address> {
    let mut result = trace
        .into_par_iter()
        .flat_map(|trace| {
            trace
                .full_trace
                .trace
                .into_par_iter()
                .filter_map(|call_frame| match call_frame.action {
                    reth_rpc_types::trace::parity::Action::Call(c) => Some(c.to),
                    reth_rpc_types::trace::parity::Action::Create(_)
                    | reth_rpc_types::trace::parity::Action::Reward(_) => None,
                    reth_rpc_types::trace::parity::Action::Selfdestruct(s) => Some(s.address),
                })
                .collect::<HashSet<_>>()
        })
        .collect::<HashSet<_>>();

    result.shrink_to_fit();
    println!("{}", result.len());

    result
}

//
//
// ------------------------ BINDINGS ------------------------
//
//

/// generates all bindings and enums for them and writes them to a file
async fn generate(bindings_file_path: &str, addresses: &Vec<(ProtocolDetails, bool, bool)>) {
    let mut file = write_file(bindings_file_path, true);

    let mut addr_bindings = Vec::new();
    let mut binding_enums = init_enum("StaticBindings", addresses.is_empty());
    let mut return_binding_enums = init_enum("StaticReturnBindings", addresses.is_empty());
    let mut mod_enums = Vec::new();
    let mut bindings_impl_try_decode = bindings_try_decode_impl_init();

    for (protocol_addr, has_functions, _has_events) in addresses {
        if !has_functions {
            continue;
        }

        let name = if protocol_addr.classifier_name.is_none() {
            protocol_addr
                .addresses
                .first()
                .map(|string| "Contract".to_string() + string)
                .unwrap()
        } else {
            protocol_addr.classifier_name.as_ref().unwrap().clone()
        };
        let name = &name;

        let abi_file_path = get_file_path(ABI_DIRECTORY, name, ".json");
        addr_bindings.push(binding_string(&abi_file_path, name));

        binding_enums.push(enum_binding_string(name, Some("_Enum")));
        return_binding_enums.push(enum_binding_string(&name, Some(&format!("::{}Calls", name))));
        individual_sub_enums(&mut mod_enums, name, addresses.is_empty());
        enum_impl_macro(&mut mod_enums, name);
        bindings_impl_try_decode.push(bindings_try_row(name));
    }

    binding_enums.push("}".to_string());
    return_binding_enums.push("}".to_string());
    bindings_impl_try_decode.push(r#"_=> panic!("no binding match found")}"#.to_string());
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
fn init_enum(name: &str, is_empty: bool) -> Vec<String> {
    let mut bindings = Vec::new();
    bindings.push("\n#[allow(non_camel_case_types)]".to_string());
    if is_empty {
        bindings.push(format!("pub enum {} {{", name));
    } else {
        bindings.push(format!("#[repr(u32)]\n pub enum {} {{", name));
    }

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
fn individual_sub_enums(mod_enum: &mut Vec<String>, protocol_name: &str, is_empty: bool) {
    let mut enum_protocol_name = protocol_name.to_string();
    enum_protocol_name.push_str("_Enum");
    mod_enum.extend(init_enum(&enum_protocol_name, is_empty));
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
fn write_all_abis(protos: &Vec<(ProtocolDetails, bool, bool)>) {
    for (protocol_addr, has_functions, _) in protos {
        if !has_functions {
            continue;
        }

        let name = if protocol_addr.classifier_name.is_none() {
            protocol_addr
                .addresses
                .first()
                .map(|string| "Contract".to_string() + string)
                .unwrap()
        } else {
            protocol_addr.classifier_name.as_ref().unwrap().clone()
        };

        let abi_file_path = get_file_path(ABI_DIRECTORY, &name, ".json");
        let mut file = write_file(&abi_file_path, true);
        let decoded: Value = serde_json::from_str(protocol_addr.abi.as_ref().unwrap()).unwrap();
        file.write_all(&serde_json::to_vec_pretty(&decoded).unwrap())
            .unwrap();
    }
}

/// creates a mapping of each address to an abi binding
fn address_abi_mapping(mapping: Vec<(ProtocolDetails, bool, bool)>) {
    let path = Path::new(&env::var("ABI_BUILD_DIR").unwrap()).join(PROTOCOL_ADDRESS_SET_PATH);
    let mut file = BufWriter::new(File::create(&path).unwrap());

    let mut used_addresses = HashSet::new();

    let mut phf_map = phf_codegen::Map::new();
    for (map, has_functions, _) in mapping {
        if !has_functions {
            continue;
        }

        if map.classifier_name.is_none() {
            let name = "Contract".to_string() + map.addresses.first().unwrap();
            writeln!(
                &mut file,
                "
                static {}: Lazy<(Option<Box<dyn ActionCollection>>,StaticBindings)> = Lazy::new(|| \
                 (None, StaticBindings::{}({}_Enum::None)));
                ",
                name.to_uppercase(),
                name,
                name
            )
            .unwrap();

            for address in map.addresses {
                if !used_addresses.insert(address.clone()) {
                    continue;
                }

                phf_map.entry(
                    H160::from_str(&address).unwrap().0,
                    &format!("&{}", name.to_uppercase()),
                );
            }
        } else {
            let name = &map.classifier_name.as_ref().unwrap().clone();
            let classified_name = map.classifier_name.as_ref().unwrap().clone() + "Classifier";
            writeln!(
                &mut file,
                "static {}: Lazy<(Option<Box<dyn ActionCollection>>,StaticBindings)> = \
                 Lazy::new(|| (Some(Box::new({}::default())), StaticBindings::{}({}_Enum::None)));",
                name.to_uppercase(),
                classified_name,
                name,
                name
            )
            .unwrap();

            for address in map.addresses {
                if !used_addresses.insert(address.clone()) {
                    continue;
                }

                phf_map.entry(
                    H160::from_str(&address).unwrap().0,
                    &format!("&{}", name.to_uppercase()),
                );
            }
        }
    }

    writeln!(
        &mut file,
        "pub static PROTOCOL_ADDRESS_MAPPING: phf::Map<[u8; 20], &'static Lazy<(Option<Box<dyn \
         ActionCollection>>,StaticBindings)>> = \n{};\n",
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

fn to_string_vec(tokens: Vec<String>) -> String {
    let tokens = tokens
        .into_iter()
        .map(|t| H160::from_str(&t).unwrap())
        .collect::<Vec<_>>();
    let mut res = "[".to_string();
    for token in tokens {
        res += "H160([";
        for byte in token.to_fixed_bytes() {
            res += &byte.to_string();
            res += ",";
        }
        let _ = res.pop();
        res += "]),";
    }
    let _ = res.pop();
    res += "]";

    res
}

fn write_test<T: std::fmt::Debug>(thing: T) {
    let file_path = Path::new(&env::var("ABI_BUILD_DIR").unwrap()).join("test_write.txt");

    let mut file = fs::OpenOptions::new()
        .append(true)
        .read(true)
        .open(&file_path)
        .expect("could not open file");

    file.write_all(format!("{:?}\n", thing).as_bytes()).unwrap(); // Added a newline for formatting
}
