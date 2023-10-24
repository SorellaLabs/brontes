use std::fs;

use brontes_types::structured_trace::{TransactionTraceWithLogs, TxTrace};
use dotenv::dotenv;
use futures::future::join_all;
use reth_primitives::H256;
use reth_rpc_types::{
    trace::parity::{TraceResults, TransactionTrace, VmTrace},
    Log, TransactionReceipt,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::mpsc::unbounded_channel;

use crate::decoding::{parser::test_utils::init_trace_parser, vm_linker::link_vm_to_trace};

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
struct TestTransactionTraceWithLogs {
    trace: TransactionTrace,
    logs:  Vec<Log>,
}

impl From<TransactionTraceWithLogs> for TestTransactionTraceWithLogs {
    fn from(value: TransactionTraceWithLogs) -> Self {
        Self { trace: value.trace, logs: value.logs }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
struct TestTxTrace {
    trace:           Vec<TestTransactionTraceWithLogs>,
    tx_hash:         H256,
    gas_used:        u64,
    effective_price: u64,
    tx_index:        u64,
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
struct TestTraceResults {
    jsonrpc: String,
    result:  TraceResults,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct TestTransactionReceipt {
    jsonrpc: String,
    result:  TransactionReceipt,
}

async fn get_full_tx_trace(tx_hash: H256) -> TraceResults {
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

async fn get_tx_reciept(tx_hash: H256) -> TransactionReceipt {
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

#[tokio::test]
async fn test_execute_block() {
    dotenv().ok();

    let (tx, _) = unbounded_channel();

    let tracer = init_trace_parser(tokio::runtime::Handle::current().clone(), tx);

    let block_1 = tracer.execute_block(17000000).await;
    assert!(block_1.is_some());

    let traces = block_1.unwrap().0;
    assert_eq!(traces.len(), 102);

    let txs: Vec<TestTxTrace> = join_all(
        traces
            .iter()
            .map(|t| async {
                let full_trace = get_full_tx_trace(t.tx_hash.clone()).await;
                let receipt = get_tx_reciept(t.tx_hash.clone()).await;

                let traces_with_logs =
                    link_vm_to_trace(full_trace.vm_trace.unwrap(), full_trace.trace, receipt.logs);

                TxTrace::new(
                    traces_with_logs,
                    receipt.transaction_hash.unwrap(),
                    receipt.transaction_index.as_u64(),
                    receipt.gas_used.unwrap().to::<u64>(),
                    receipt.effective_gas_price.to::<u64>(),
                )
                .into()
            })
            .collect::<Vec<_>>(),
    )
    .await;

    let t1 = txs[0].clone();
    let t2 = traces[0].clone().into();
    assert_eq!(t1, t2);

    assert_eq!(txs, traces.into_iter().map(|t| t.into()).collect::<Vec<_>>())
}

#[test]
fn test_link_vm_to_trace() {
    // Load the trace and receipt from the JSON files
    let trace_json: TestTraceResults = serde_json::from_str(
        &fs::read_to_string(
            "src/test_utils/0x380e6cda70b04f647a40c07e71a154e9af94facb13dc5f49c2556497ec34d6f0/\
             trace.json",
        )
        .unwrap(),
    )
    .unwrap();
    let receipt_json: TestTransactionReceipt = serde_json::from_str(
        &fs::read_to_string(
            "src/test_utils/0x380e6cda70b04f647a40c07e71a154e9af94facb13dc5f49c2556497ec34d6f0/\
             receipt.json",
        )
        .unwrap(),
    )
    .unwrap();

    // Deserialize the JSON into the appropriate data structures
    let vm_trace: VmTrace = trace_json.result.vm_trace.unwrap();
    let tx_trace: Vec<TransactionTrace> = trace_json.result.trace;
    let logs: Vec<Log> = receipt_json.result.logs;

    let current_traces = link_vm_to_trace(vm_trace.clone(), tx_trace.clone(), logs.clone());

    // Check that the function correctly parsed the traces
    assert_eq!(current_traces.len(), tx_trace.len());

    for trace_with_logs in current_traces.iter() {
        assert!(tx_trace.contains(&trace_with_logs.trace));

        let with_logs = vec![vec![0, 0, 2, 0, 0], vec![0, 0, 1], vec![0, 0, 0], vec![0, 0, 2]];

        if with_logs.contains(&trace_with_logs.trace.trace_address) {
            trace_with_logs
                .logs
                .clone()
                .into_iter()
                .for_each(|log| assert!(logs.contains(&log)))
        } else {
            assert!(trace_with_logs.logs.is_empty())
        }
    }
}
