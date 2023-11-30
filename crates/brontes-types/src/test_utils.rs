use reth_rpc_types::trace::parity::{
    CallAction, TraceResultsWithTransactionHash, TransactionTrace,
};
use sorella_db_databases::ClickhouseClient;

use crate::{normalized_actions::Actions, tree::TimeTree};

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

pub fn force_call_action(trace: &TransactionTrace) -> CallAction {
    match &trace.action {
        reth_rpc_types::trace::parity::Action::Call(c) => c.clone(),
        _ => unreachable!(),
    }
}

pub fn force_call_action_outer(trace: &TraceResultsWithTransactionHash, idx: usize) -> CallAction {
    let inner_trace = &trace.full_trace.trace[idx];
    force_call_action(inner_trace)
}
