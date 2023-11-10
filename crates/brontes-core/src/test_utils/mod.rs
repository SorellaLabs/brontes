use std::{
    env, fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use alloy_etherscan::Client;
use brontes_metrics::PoirotMetricEvents;
use brontes_types::structured_trace::{TransactionTraceWithLogs, TxTrace};
use dotenv::dotenv;
use ethers_core::types::Chain;
use futures::future::join_all;
use reqwest::Url;
use reth_primitives::H256;
use reth_rpc_types::{
    trace::parity::{TraceResults, TransactionTrace, VmTrace},
    Log, TransactionReceipt,
};
use reth_tracing::TracingClient;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::{
    runtime::Handle,
    sync::mpsc::{unbounded_channel, UnboundedSender},
};

use crate::decoding::{parser::TraceParser, TracingProvider, CACHE_DIRECTORY, CACHE_TIMEOUT};

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct TestTransactionTraceWithLogs {
    pub trace: TransactionTrace,
    pub logs:  Vec<Log>,
}

impl From<TransactionTraceWithLogs> for TestTransactionTraceWithLogs {
    fn from(value: TransactionTraceWithLogs) -> Self {
        Self { trace: value.trace, logs: value.logs }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct TestTxTrace {
    pub trace:           Vec<TestTransactionTraceWithLogs>,
    pub tx_hash:         H256,
    pub gas_used:        u64,
    pub effective_price: u64,
    pub tx_index:        u64,
}

impl From<TxTrace> for TestTxTrace {
    fn from(value: TxTrace) -> Self {
        Self {
            trace:           value.trace.into_iter().map(|v| v.into()).collect(),
            tx_hash:         value.tx_hash,
            gas_used:        value.gas_used,
            effective_price: value.effective_price,
            tx_index:        value.tx_index,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TestTraceResults {
    pub jsonrpc: String,
    pub result:  TraceResults,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TestTransactionReceipt {
    pub jsonrpc: String,
    pub result:  TransactionReceipt,
}

pub async fn get_full_tx_trace(tx_hash: H256) -> TraceResults {
    let url = "https://reth.sorella-beechit.com:8489";
    let headers = reqwest::header::HeaderMap::from_iter(
        vec![(reqwest::header::CONTENT_TYPE, "application/json".parse().unwrap())].into_iter(),
    );

    let payload = json!({
        "id": 1,
        "jsonrpc": "2.0",
        "method": "trace_replayTransaction",
        "params": [&format!("{:#x}", &tx_hash), ["trace", "vmTrace"]]
    });

    let client = reqwest::Client::new();
    let response: TestTraceResults = client
        .post(url)
        .headers(headers)
        .json(&payload)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    response.result
}

pub async fn get_tx_reciept(tx_hash: H256) -> TransactionReceipt {
    let url = "https://reth.sorella-beechit.com:8489";
    let headers = reqwest::header::HeaderMap::from_iter(
        vec![(reqwest::header::CONTENT_TYPE, "application/json".parse().unwrap())].into_iter(),
    );

    let payload = json!({
        "id": 1,
        "jsonrpc": "2.0",
        "method": "eth_getTransactionReceipt",
        "params": [&format!("{:#x}", &tx_hash)]
    });

    let client = reqwest::Client::new();
    let response: TestTransactionReceipt = client
        .post(url)
        .headers(headers)
        .json(&payload)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    response.result
}

use tracing::Level;
pub fn init_tracing() {
    let filter = EnvFilter::builder()
        .with_default_directive(Level::DEBUG.into())
        .from_env_lossy();

    let subscriber = Registry::default().with(tracing_subscriber::fmt::layer().with_filter(filter));
}
use tracing_subscriber::{
    fmt, prelude::__tracing_subscriber_SubscriberExt, EnvFilter, Layer, Registry,
};

// TODO: Joe pls fix, fyi you had the on above before
pub fn init_tracing() {
    // Setup a filter for tracing
    let filter = EnvFilter::builder()
        .with_default_directive(Level::INFO.into()) // Sets the default level to TRACE
        .from_env_lossy(); // Tries to get the log level directive from RUST_LOG env var

    // Setup the subscriber
    let subscriber = Registry::default().with(fmt::layer().with_filter(filter)); // Attach the filter to the formatter layer

    // Set the subscriber as the global default (may fail if already set in another
    // test)
    if tracing::subscriber::set_global_default(subscriber).is_err() {
        eprintln!(
            "Warning: Could not set the tracing subscriber as the global default (it may already \
             be set)"
        );
    }
}

pub fn init_trace_parser(
    handle: Handle,
    metrics_tx: UnboundedSender<PoirotMetricEvents>,
) -> TraceParser<Box<dyn TracingProvider>> {
    let etherscan_key = env::var("ETHERSCAN_API_KEY").expect("No ETHERSCAN_API_KEY in .env");
    let db_path = env::var("DB_PATH").expect("No DB_PATH in .env");

    let etherscan_client = Client::new_cached(
        Chain::Mainnet,
        etherscan_key,
        Some(PathBuf::from(CACHE_DIRECTORY)),
        CACHE_TIMEOUT,
    )
    .unwrap();

    #[cfg(feature = "local")]
    let tracer = {
        let db_endpoint = env::var("RETH_ENDPOINT").expect("No db Endpoint in .env");
        let db_port = env::var("RETH_PORT").expect("No DB port.env");
        let url = format!("{db_endpoint}:{db_port}");
        Box::new(ethers::providers::Provider::new(ethers::providers::Http::new(
            url.parse::<Url>().unwrap(),
        ))) as Box<dyn TracingProvider>
    };

    #[cfg(not(feature = "local"))]
    let tracer = {
        Box::new(TracingClient::new(Path::new(&db_path), handle.clone()))
            as Box<dyn TracingProvider>
    };

    TraceParser::new(etherscan_client, Arc::new(tracer), Arc::new(metrics_tx))
}
