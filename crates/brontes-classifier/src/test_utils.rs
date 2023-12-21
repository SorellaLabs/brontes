use brontes_core::decoding::{parser::TraceParser, TracingProvider};
use brontes_database::{clickhouse::Clickhouse, Metadata};
use brontes_database_libmdbx::Libmdbx;
use brontes_types::{normalized_actions::Actions, structured_trace::TxTrace, tree::TimeTree};
use reth_primitives::Header;

use crate::Classifier;

pub fn helper_build_tree(
    classifier: &Classifier,
    traces: Vec<TxTrace>,
    header: Header,
    metadata: &Metadata,
) -> TimeTree<Actions> {
    let (_, mut tree) = classifier.build_tree(traces, header);
    tree.eth_price = metadata.eth_prices.clone();
    tree
}

pub async fn build_raw_test_tree<T: TracingProvider>(
    tracer: &TraceParser<'_, T>,
    db: &Clickhouse,
    lib: &Libmdbx,
    block_number: u64,
) -> TimeTree<Actions> {
    let (traces, header) = get_traces_with_meta(tracer, db, block_number).await;
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    let classifier = Classifier::new(&lib, tx);
    let (_, tree) = classifier.build_tree(traces, header);
    tree
}

pub async fn get_traces_with_meta<T: TracingProvider>(
    tracer: &TraceParser<'_, T>,
    db: &Clickhouse,
    block_number: u64,
) -> (Vec<TxTrace>, Header) {
    let (traces, header) = tracer.execute_block(block_number).await.unwrap();
    (traces, header)
}
