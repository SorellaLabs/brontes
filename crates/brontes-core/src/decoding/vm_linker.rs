use alloy_primitives::U256;
use brontes_types::structured_trace::TransactionTraceWithLogs;
use reth_primitives::{Address, Bytes, H256};
use reth_rpc_types::{
    trace::parity::{TransactionTrace, VmInstruction, VmTrace},
    Log,
};

pub fn link_vm_to_trace(
    vm: VmTrace,
    mut tx_trace: Vec<TransactionTrace>,
) -> Vec<TransactionTraceWithLogs> {
    let mut res = Vec::new();
    recursive_parsing(&mut res, vm, &mut tx_trace);

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
    push_stack: &mut Vec<U256>,
    instruction: &mut VmInstruction,
    current_address: Address,
) -> Option<Log> {
    // NOTE: this might be Log0 instead but we go with this code
    match instruction.op.take()?.as_str() {
        "A0" => {
            let delta = instruction.ex.as_mut()?.mem.take()?;
            let bytes = delta.data;

            Some(Log {
                address:           current_address,
                data:              bytes,
                topics:            vec![],
                removed:           false,
                log_index:         None,
                block_hash:        None,
                block_number:      None,
                transaction_hash:  None,
                transaction_index: None,
            })
        }
        "A1" => {
            let delta = instruction.ex.as_mut()?.mem.take()?;
            let bytes = delta.data;
            let topic0 = push_stack.remove(0);
            Some(Log {
                address:           current_address,
                data:              bytes,
                topics:            vec![topic0.into()],
                removed:           false,
                log_index:         None,
                block_hash:        None,
                block_number:      None,
                transaction_hash:  None,
                transaction_index: None,
            })
        }
        "A2" => {
            let delta = instruction.ex.as_mut()?.mem.take()?;
            let bytes = delta.data;
            let topic1 = push_stack.remove(0);
            let topic2 = push_stack.remove(1);

            Some(Log {
                address:           current_address,
                data:              bytes,
                topics:            vec![topic1.into(), topic2.into()],
                removed:           false,
                log_index:         None,
                block_hash:        None,
                block_number:      None,
                transaction_hash:  None,
                transaction_index: None,
            })
        }
        "A3" => {
            let delta = instruction.ex.as_mut()?.mem.take()?;
            let bytes = delta.data;
            let topic1 = push_stack.remove(0);
            let topic2 = push_stack.remove(1);
            let topic3 = push_stack.remove(2);

            Some(Log {
                address:           current_address,
                data:              bytes,
                topics:            vec![topic1.into(), topic2.into(), topic3.into()],
                removed:           false,
                log_index:         None,
                block_hash:        None,
                block_number:      None,
                transaction_hash:  None,
                transaction_index: None,
            })
        }
        "A4" => {
            let delta = instruction.ex.as_mut()?.mem.take()?;
            let bytes = delta.data;
            let topic1 = push_stack.remove(0);
            let topic2 = push_stack.remove(1);
            let topic3 = push_stack.remove(2);
            let topic4 = push_stack.remove(3);

            Some(Log {
                address:           current_address,
                data:              bytes,
                topics:            vec![topic1.into(), topic2.into(), topic3.into(), topic4.into()],
                removed:           false,
                log_index:         None,
                block_hash:        None,
                block_number:      None,
                transaction_hash:  None,
                transaction_index: None,
            })
        }

        _ => return None,
    };
    None
}

fn recursive_parsing(
    current_traces: &mut Vec<TransactionTraceWithLogs>,
    vm: VmTrace,
    tx_trace: &mut Vec<TransactionTrace>,
) {
    let scoped_trace = tx_trace.remove(0);
    // NOTE: this doesn't work
    let mut only_push_stack = Vec::new();

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
                recursive_parsing(current_traces, sub, tx_trace)
            }

            let res = try_parse(&mut only_push_stack, &mut instruction, addr);
            if let Some(ex) = instruction.ex.take() {
                only_push_stack.extend(ex.push);
            }

            res
        })
        .collect::<Vec<Log>>();

    current_traces.push(TransactionTraceWithLogs { trace: scoped_trace, logs })
}
