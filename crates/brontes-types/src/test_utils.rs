use reth_primitives::Address;
use reth_rpc_types::trace::parity::{
    CallAction, TraceResultsWithTransactionHash, TransactionTrace,
};
use sorella_db_databases::ClickhouseClient;

use crate::{
    normalized_actions::Actions,
    tree::{Node, TimeTree},
};

pub fn spawn_db() -> ClickhouseClient {
    ClickhouseClient::default()
}

pub fn print_tree_as_json(tree: &TimeTree<Actions>) {
    let serialized_tree = serde_json::to_string_pretty(tree).unwrap();
    println!("{}", serialized_tree);
}

pub async fn write_tree_as_json(tree: &TimeTree<Actions>, path: &str) {
    let serialized_tree = serde_json::to_string_pretty(tree).unwrap();
    tokio::fs::write(path, serialized_tree).await.unwrap();
}

#[derive(Debug, PartialEq, Eq)]
pub struct ComparisonNode {
    inner_len: usize,
    finalized: bool,
    index: u64,
    subactions_len: usize,
    trace_address: Vec<usize>,
    address: Address,
    trace: TransactionTrace,
}

impl ComparisonNode {
    pub fn new(trace: &TransactionTrace, index: usize, inner_len: usize) -> Self {
        Self {
            inner_len,
            finalized: false,
            index: index as u64,
            subactions_len: 0,
            trace_address: trace.trace_address.clone(),
            address: force_call_action(trace).from,
            trace: trace.clone(),
        }
    }
}

impl From<&Node<Actions>> for ComparisonNode {
    fn from(value: &Node<Actions>) -> Self {
        ComparisonNode {
            inner_len: value.inner.len(),
            finalized: value.finalized,
            index: value.index,
            subactions_len: value.subactions.len(),
            trace_address: value.trace_address.clone(),
            address: value.address,
            trace: match &value.data {
                Actions::Unclassified(traces, _) => traces.trace.clone(),
                _ => unreachable!(),
            },
        }
    }
}

fn force_call_action(trace: &TransactionTrace) -> CallAction {
    match &trace.action {
        reth_rpc_types::trace::parity::Action::Call(c) => c.clone(),
        _ => unreachable!(),
    }
}

pub fn force_call_action_outer(trace: &TraceResultsWithTransactionHash, idx: usize) -> CallAction {
    let inner_trace = &trace.full_trace.trace[idx];
    force_call_action(inner_trace)
}
