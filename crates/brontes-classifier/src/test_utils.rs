use brontes_core::decoding::{parser::TraceParser, TracingProvider};
use brontes_database::{clickhouse::Clickhouse, Metadata};
use brontes_database_libmdbx::Libmdbx;
use brontes_types::{normalized_actions::Actions, structured_trace::TxTrace, tree::BlockTree};
use reth_primitives::Header;

use crate::Classifier;

pub async fn helper_build_block_tree<T: TracingProvider>(
    classifier: &Classifier<'_, T>,
    traces: Vec<TxTrace>,
    header: Header,
    metadata: &Metadata,
) -> BlockTree<Actions> {
    let (_, mut tree) = classifier.build_block_tree(traces, header).await;
    tree.eth_price = metadata.eth_prices.clone();
    tree
}

pub async fn build_raw_test_tree<T: TracingProvider>(
    tracer: &TraceParser<'_, T>,
    db: &Clickhouse,
    lib: &Libmdbx,
    block_number: u64,
) -> BlockTree<Actions> {
    let (traces, header) = get_traces_with_meta(tracer, db, block_number).await;
    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    let classifier = Classifier::new(&lib, tx, tracer.get_tracer());
    let (_, tree) = classifier.build_block_tree(traces, header).await;
    tree
}

pub async fn get_traces_with_meta<T: TracingProvider>(
    tracer: &TraceParser<'_, T>,
    _db: &Clickhouse,
    block_number: u64,
) -> (Vec<TxTrace>, Header) {
    let (traces, header) = tracer.execute_block(block_number).await.unwrap();
    (traces, header)
}
