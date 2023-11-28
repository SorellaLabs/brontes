use brontes_types::structured_trace::TransactionTraceWithLogs;
use reth_rpc_types::{
    trace::parity::{TransactionTrace, VmInstruction, VmTrace},
    Log,
};

pub fn link_vm_to_trace(
    vm: VmTrace,
    tx_trace: Vec<TransactionTrace>,
    mut logs: Vec<Log>,
) -> Vec<TransactionTraceWithLogs> {
    let mut res = Vec::new();
    recursive_parsing(
        &mut res,
        vm,
        &mut tx_trace
            .into_iter()
            .enumerate()
            .map(|ti| ti)
            .collect::<Vec<_>>(),
        &mut logs,
    );
    res.sort_by_key(|item| item.trace_idx);

    res
}

/// all type of log setups
/// Log0 { offset: Bytes, size: Bytes },
/// Log1 { offset: Bytes, size: Bytes, topic: H256 },
/// Log2 { offset: Bytes, size: Bytes, topic1: H256, topic2: H256 },
/// Log3 { offset: Bytes, size: Bytes, topic1: H256, topic2: H256, topic3: H256
/// }, Log4 { offset: Bytes, size: Bytes, topic1: H256, topic2: H256, topic3:
/// H256, topic4: H256 },
fn try_parse(mut instruction: VmInstruction, logs: &mut Vec<Log>) -> Option<Log> {
    match instruction.op.take()?.as_str() {
        "LOG0" | "LOG1" | "LOG2" | "LOG3" | "LOG4" => {
            if logs.len() == 0 {
                return None
            } else {
                Some(logs.remove(0))
            }
        }
        _ => None,
    }
}

/// this currently breaks if a log is emitted after it calls a new tx that emits
/// a tx
fn recursive_parsing(
    current_traces: &mut Vec<TransactionTraceWithLogs>,
    vm: VmTrace,
    tx_trace: &mut Vec<(usize, TransactionTrace)>,
    logs: &mut Vec<Log>,
) {
    let (idx, scoped_trace) = tx_trace.remove(0);

    let logs = vm
        .ops
        .into_iter()
        .filter_map(|mut instruction| {
            if let Some(sub) = instruction.sub.take() {
                recursive_parsing(current_traces, sub, tx_trace, logs)
            }

            try_parse(instruction, logs)
        })
        .collect::<Vec<Log>>();

    current_traces.push(TransactionTraceWithLogs {
        trace: scoped_trace,
        decoded_data: None,
        logs,
        trace_idx: idx as u64,
    })
}

#[cfg(test)]
mod tests {

    use std::{fs, str::FromStr};

    use reth_primitives::BlockNumberOrTag;
    use reth_rpc_types::TransactionReceipt;

    use super::*;
    use crate::test_utils::*;

    #[tokio::test]
    async fn print_logs() {
        dotenv::dotenv().ok();
        init_tracing();

        let tracer = init_trace_parser(tokio::runtime::Handle::current().clone(), tx);
        let hash = reth_primitives::H256::from_str(
            "0x22ea36d516f59cc90ccc01042e20f8fba196f32b067a7e5f1510099140ae5e0a",
        )
        .unwrap();

        let tx_receipts: Vec<TransactionReceipt> = tracer
            .tracer
            .block_receipts(BlockNumberOrTag::Number(18539312))
            .await
            .unwrap()
            .unwrap();

        let receipt = tx_receipts
            .into_iter()
            .find(|r| r.transaction_hash.unwrap() == hash)
            .unwrap();
        println!("{:#?}", receipt.logs);
    }

    #[test]
    fn test_link_vm_to_trace() {
        // Load the trace and receipt from the JSON files
        let trace_json: TestTraceResults = serde_json::from_str(
            &fs::read_to_string(
                "src/test_utils/\
                 0x380e6cda70b04f647a40c07e71a154e9af94facb13dc5f49c2556497ec34d6f0/trace.json",
            )
            .unwrap(),
        )
        .unwrap();
        let receipt_json: TestTransactionReceipt = serde_json::from_str(
            &fs::read_to_string(
                "src/test_utils/\
                 0x380e6cda70b04f647a40c07e71a154e9af94facb13dc5f49c2556497ec34d6f0/receipt.json",
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
}
