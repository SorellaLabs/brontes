use std::collections::{HashMap, HashSet};

use brontes_core::decoding::{parser::TraceParser, TracingProvider};
use brontes_database::{database::Database, Metadata};
use brontes_types::{
    normalized_actions::{
        Actions, NormalizedBurn, NormalizedMint, NormalizedSwap, NormalizedTransfer,
    },
    structured_trace::{TraceActions, TransactionTraceWithLogs, TxTrace},
    tree::{GasDetails, Node, Root, TimeTree},
};
use hex_literal::hex;
use parking_lot::RwLock;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use reth_primitives::{Address, Header, H256, U256};
use reth_rpc_types::{trace::parity::Action, Log};
use reth_tracing::TracingClient;

use crate::{Classifier, StaticReturnBindings, PROTOCOL_ADDRESS_MAPPING};

const TRANSFER_TOPIC: H256 =
    H256(hex!("ddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef"));

pub fn helper_build_tree(
    classifier: &Classifier,
    traces: Vec<TxTrace>,
    header: Header,
    metadata: &Metadata,
) -> TimeTree<Actions> {
    classifier.build_tree(traces, header, metadata)
}

pub async fn build_raw_test_tree<T: TracingProvider>(
    tracer: &TraceParser<'_, T>,
    db: &Database,
    block_number: u64,
) -> TimeTree<Actions> {
    let (traces, header, metadata) = get_traces_with_meta(tracer, db, block_number).await;
    let classifier = Classifier::new();
    classifier.build_tree(traces, header, &metadata)
}

pub async fn get_traces_with_meta<T: TracingProvider>(
    tracer: &TraceParser<'_, T>,
    db: &Database,
    block_number: u64,
) -> (Vec<TxTrace>, Header, Metadata) {
    let (traces, header) = tracer.execute_block(block_number).await.unwrap();
    let metadata = db.get_metadata(block_number).await;
    (traces, header, metadata)
}
