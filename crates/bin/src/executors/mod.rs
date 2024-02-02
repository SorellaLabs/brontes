mod range;
mod shared;
mod tip;

use std::{
    pin::Pin,
    task::{Context, Poll},
};

use alloy_primitives::Address;
use brontes_classifier::Classifier;
use brontes_core::{
    decoding::{Parser, TracingProvider},
    LibmdbxReadWriter,
};
use brontes_database::{
    clickhouse::Clickhouse,
    libmdbx::{LibmdbxReader, LibmdbxWriter},
};
use brontes_inspect::Inspector;
use brontes_pricing::types::DexPriceMsg;
use futures::{pin_mut, stream::FuturesUnordered, Future, FutureExt, StreamExt};
pub use range::RangeExecutorWithPricing;
use reth_tasks::{shutdown::GracefulShutdown, TaskExecutor};
pub use tip::TipInspector;
use tokio::sync::mpsc::UnboundedReceiver;
use tracing::info;

pub const PROMETHEUS_ENDPOINT_IP: [u8; 4] = [127u8, 0u8, 0u8, 1u8];
pub const PROMETHEUS_ENDPOINT_PORT: u16 = 6423;

#[derive(Debug)]
pub struct BrontesRunConfig<T: TracingProvider, DB: LibmdbxReader + LibmdbxWriter> {
    pub task_executor: TaskExecutor,
    pub start_block:   u64,
    pub end_block:     Option<u64>,

    pub max_tasks:        u64,
    pub quote_asset:      Address,
    pub with_dex_pricing: bool,
    pub init_libmdbx:     bool,

    pub inspectors:       &'static [&'static Box<dyn Inspector>],
    pub clickhouse:       Option<&'static Clickhouse>,
    pub libmdbx:          &'static DB,
    pub tracing_provider: Arc<T>,
}

impl<T: TracingProvider, DB: LibmdbxWriter + LibmdbxReader> BrontesRunConfig<T, DB> {}

pub struct Brontes<T: TracingProvider, DB: LibmdbxReader + LibmdbxWriter> {
    config: BrontesRunConfig<T, DB>,
}

impl<T: TracingProvider, DB: LibmdbxReader + LibmdbxWriter> Brontes<T, DB> {}

impl<T: TracingProvider, DB: LibmdbxReader + LibmdbxWriter> Future for Brontes<T, DB> {
    type Output = Option<TipInspector<T, DB>>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {}
}

