use std::env;

use brontes_core::decoding::{parser::TraceParser, TracingProvider};
use brontes_database::{clickhouse::Clickhouse, Metadata};
use brontes_database_libmdbx::Libmdbx;
use brontes_pricing::types::DexPrices;
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
    block_number: u64,
) -> TimeTree<Actions> {
    let (traces, header, metadata) = get_traces_with_meta(tracer, db, block_number).await;
    let brontes_db_endpoint = env::var("BRONTES_DB_PATH").expect("No BRONTES_DB_PATH in .env");
    let libmdbx = Libmdbx::init_db(brontes_db_endpoint, None).unwrap();
    let (tx, _rx) = tokio::sync::mpsc::channel(5);
    let classifier = Classifier::new(&libmdbx, tx);
    let (_, mut tree) = classifier.build_tree(traces, header);
    tree.eth_price = metadata.eth_prices.clone();
    tree
}

pub async fn get_traces_with_meta<T: TracingProvider>(
    tracer: &TraceParser<'_, T>,
    db: &Clickhouse,
    block_number: u64,
) -> (Vec<TxTrace>, Header, Metadata) {
    let (traces, header) = tracer.execute_block(block_number).await.unwrap();
    let metadata = db.get_metadata(block_number).await;
    let metadata = metadata.into_finalized_metadata(DexPrices::new());
    (traces, header, metadata)
}
