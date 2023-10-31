use std::collections::HashSet;

use brontes_classifier::test_utils::build_raw_test_tree;
use brontes_core::{decoding::parser::TraceParser, test_utils::init_trace_parser};
use brontes_database::database::Database;
use brontes_metrics::PoirotMetricEvents;
use brontes_types::{normalized_actions::Actions, test_utils::*, tree::TimeTree};
use reth_rpc::eth::EthTransactions;
use reth_rpc_types::trace::parity::{
    Action, CallAction, TraceResultsWithTransactionHash, TraceType,
};
use reth_tracing::TracingClient;
use serial_test::serial;
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};

use crate::UNIT_TESTS_BLOCK_NUMBER;

#[tokio::test]
#[serial]
async fn test_raw_tree() {
    dotenv::dotenv().ok();

    let (tx, _rx) = unbounded_channel();

    let tracer = init_trace_parser(tokio::runtime::Handle::current().clone(), tx);
    let db = Database::default();
    let mut tree = build_raw_test_tree(&tracer, db, UNIT_TESTS_BLOCK_NUMBER).await;

    let mut transaction_traces = tracer
        .tracer
        .trace
        .replay_block_transactions(
            UNIT_TESTS_BLOCK_NUMBER.into(),
            HashSet::from([TraceType::Trace]),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(tree.roots.len(), transaction_traces.len());

    let first_root = tree.roots.remove(0);
    let first_tx = transaction_traces.remove(0);

    assert_eq!(
        Into::<ComparisonNode>::into(&first_root.head),
        ComparisonNode::new(&first_tx.full_trace.trace[0], 0, 8)
    );

    assert_eq!(
        Into::<ComparisonNode>::into(&first_root.head.inner[0]),
        ComparisonNode::new(&first_tx.full_trace.trace[1], 1, 1)
    );

    assert_eq!(
        Into::<ComparisonNode>::into(&first_root.head.inner[0].inner[0]),
        ComparisonNode::new(&first_tx.full_trace.trace[2], 2, 0)
    );

    assert_eq!(
        Into::<ComparisonNode>::into(&first_root.head.inner[1]),
        ComparisonNode::new(&first_tx.full_trace.trace[3], 3, 0)
    );

    assert_eq!(
        Into::<ComparisonNode>::into(&first_root.head.inner[2]),
        ComparisonNode::new(&first_tx.full_trace.trace[4], 4, 0)
    );

    assert_eq!(
        Into::<ComparisonNode>::into(&first_root.head.inner[3]),
        ComparisonNode::new(&first_tx.full_trace.trace[5], 5, 0)
    );

    assert_eq!(
        Into::<ComparisonNode>::into(&first_root.head.inner[4]),
        ComparisonNode::new(&first_tx.full_trace.trace[6], 6, 0)
    );

    assert_eq!(
        Into::<ComparisonNode>::into(&first_root.head.inner[5]),
        ComparisonNode::new(&first_tx.full_trace.trace[7], 7, 3)
    );

    assert_eq!(
        Into::<ComparisonNode>::into(&first_root.head.inner[5].inner[0]),
        ComparisonNode::new(&first_tx.full_trace.trace[8], 8, 0)
    );

    assert_eq!(
        Into::<ComparisonNode>::into(&first_root.head.inner[5].inner[1]),
        ComparisonNode::new(&first_tx.full_trace.trace[9], 9, 0)
    );

    assert_eq!(
        Into::<ComparisonNode>::into(&first_root.head.inner[5].inner[2]),
        ComparisonNode::new(&first_tx.full_trace.trace[10], 10, 3)
    );

    assert_eq!(
        Into::<ComparisonNode>::into(&first_root.head.inner[5].inner[2].inner[0]),
        ComparisonNode::new(&first_tx.full_trace.trace[11], 11, 0)
    );

    assert_eq!(
        Into::<ComparisonNode>::into(&first_root.head.inner[5].inner[2].inner[1]),
        ComparisonNode::new(&first_tx.full_trace.trace[12], 12, 0)
    );

    assert_eq!(
        Into::<ComparisonNode>::into(&first_root.head.inner[5].inner[2].inner[2]),
        ComparisonNode::new(&first_tx.full_trace.trace[13], 13, 0)
    );

    assert_eq!(
        Into::<ComparisonNode>::into(&first_root.head.inner[6]),
        ComparisonNode::new(&first_tx.full_trace.trace[14], 14, 0)
    );

    assert_eq!(
        Into::<ComparisonNode>::into(&first_root.head.inner[7]),
        ComparisonNode::new(&first_tx.full_trace.trace[15], 15, 0)
    );
}
