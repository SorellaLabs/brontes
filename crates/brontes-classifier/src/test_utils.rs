use brontes_core::{
    decoding::{parser::TraceParser, TracingProvider},
    TraceLoader,
};
use brontes_database::{clickhouse::Clickhouse, Metadata};
use brontes_database_libmdbx::Libmdbx;
use brontes_pricing::types::DexPriceMsg;
use brontes_types::{normalized_actions::Actions, structured_trace::TxTrace, tree::BlockTree};
use reth_primitives::Header;
use tokio::sync::mpsc::UnboundedReceiver;

use crate::Classifier;

/// Classifier specific functionality
pub struct ClassifierTestUtils {
    trace_loader: TraceLoader,

    dex_pricing_receiver: UnboundedReceiver<DexPriceMsg>,
    classifier: Classifier<'static, Box<dyn TracingProvider>>


}
