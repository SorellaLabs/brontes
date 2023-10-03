use brontes_types::structured_trace::TransactionTraceWithLogs;
use reth_primitives::Address;
use reth_rpc_types::{
    trace::parity::{TransactionTrace, VmInstruction, VmTrace},
    Log,
};

pub fn link_vm_to_trace(
    vm: VmTrace,
    mut tx_trace: Vec<TransactionTrace>,
    mut logs: Vec<Log>,
) -> Vec<TransactionTraceWithLogs> {
    let mut res = Vec::new();
    recursive_parsing(&mut res, vm, &mut tx_trace, &mut logs);

    res
}

/// all type of log setups
/// Log0 { offset: Bytes, size: Bytes },
/// Log1 { offset: Bytes, size: Bytes, topic: H256 },
/// Log2 { offset: Bytes, size: Bytes, topic1: H256, topic2: H256 },
/// Log3 { offset: Bytes, size: Bytes, topic1: H256, topic2: H256, topic3: H256
/// }, Log4 { offset: Bytes, size: Bytes, topic1: H256, topic2: H256, topic3:
/// H256, topic4: H256 },
fn try_parse(
    mut instruction: VmInstruction,
    current_address: Address,
    logs: &mut Vec<Log>,
) -> Option<Log> {
    // NOTE: this might be Log0 instead but we go with this code
    match instruction.op.take()?.as_str() {
        "A0" | "A1" | "A2" | "A3" | "A4" => Some(logs.remove(0)),
        _ => None,
    }
}

fn recursive_parsing(
    current_traces: &mut Vec<TransactionTraceWithLogs>,
    vm: VmTrace,
    tx_trace: &mut Vec<TransactionTrace>,
    logs: &mut Vec<Log>,
) {
    let scoped_trace = tx_trace.remove(0);

    let logs = vm
        .ops
        .into_iter()
        .zip(vec![&scoped_trace].into_iter().cycle())
        .filter_map(|(mut instruction, trace)| {
            let addr = match &trace.action {
                reth_rpc_types::trace::parity::Action::Call(c) => c.to,
                _ => return None,
            };

            if let Some(sub) = instruction.sub.take() {
                recursive_parsing(current_traces, sub, tx_trace, logs)
            }

            let res = try_parse(instruction, addr, logs);

            res
        })
        .collect::<Vec<Log>>();

    current_traces.push(TransactionTraceWithLogs { trace: scoped_trace, logs })
}
