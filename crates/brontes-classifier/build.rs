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
        let name = &map.classifier_name.as_ref().unwrap().clone();
        let classified_name = map.classifier_name.as_ref().unwrap().clone() + "Classifier";
        phf_map.entry(name, &format!("Lazy::new(|| Box::new({}::default()))", classified_name));
    }

    writeln!(
        &mut file,
        "pub static PROTOCOL_CLASSIFIER_MAPPING: phf::Map<&'static str, Lazy<Box<dyn \
         ActionCollection>>> = \n{};\n",
        phf_map.build()
    )
    .unwrap();
}
