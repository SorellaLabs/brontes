use std::{collections::HashMap, ops::Deref};

use alloy_primitives::{Address, TxHash};
use brontes_core::{
    decoding::TracingProvider, missing_decimals::load_missing_decimals, BlockTracesWithHeaderAnd,
    TraceLoader, TraceLoaderError, TxTracesWithHeaderAnd,
};
use brontes_database::libmdbx::{LibmdbxReadWriter, LibmdbxReader};
use brontes_pricing::{types::DexPriceMsg, BrontesBatchPricer, GraphManager};
use brontes_types::{
    db::dex::DexQuotes,
    tree::{BlockTree, Node},
};
use futures::{future::join_all, StreamExt};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};

use crate::{Actions, Classifier};

pub struct ClassifierBenchUtils {
    trace_loader:         TraceLoader,
    classifier:           Classifier<'static, Box<dyn TracingProvider>, LibmdbxReadWriter>,
    dex_pricing_receiver: UnboundedReceiver<DexPriceMsg>,
    rt:                   tokio::runtime::Runtime,
}
impl ClassifierBenchUtils {
    pub fn new() -> Self {
        let trace_loader = TraceLoader::new();
        let (tx, rx) = unbounded_channel();
        let classifier = Classifier::new(trace_loader.libmdbx, tx, trace_loader.get_provider());
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();

        Self { classifier, trace_loader, dex_pricing_receiver: rx, rt }
    }

    // pub async fn bench_tx(&self, tx: TxHash,
}
