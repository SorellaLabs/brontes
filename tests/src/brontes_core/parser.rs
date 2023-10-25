use std::fs;

use brontes_core::{decoding::vm_linker::link_vm_to_trace, test_utils::*};
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
use serial_test::serial;
use tokio::sync::mpsc::unbounded_channel;

#[tokio::test]
#[serial]
async fn test_execute_block() {
    dotenv().ok();

    let (tx, _rx) = unbounded_channel();

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

    assert_eq!(txs, traces.into_iter().map(|t| t.into()).collect::<Vec<_>>())
}

#[test]
fn test_link_vm_to_trace() {
    // Load the trace and receipt from the JSON files
    let trace_json: TestTraceResults = serde_json::from_str(
        &fs::read_to_string(
            "src/brontes_core/0x380e6cda70b04f647a40c07e71a154e9af94facb13dc5f49c2556497ec34d6f0/\
             trace.json",
        )
        .unwrap(),
    )
    .unwrap();
    let receipt_json: TestTransactionReceipt = serde_json::from_str(
        &fs::read_to_string(
            "src/brontes_core/0x380e6cda70b04f647a40c07e71a154e9af94facb13dc5f49c2556497ec34d6f0/\
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
