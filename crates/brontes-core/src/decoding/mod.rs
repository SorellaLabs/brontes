use std::{collections::HashSet, path::PathBuf, pin::Pin, sync::Arc};

use reqwest::Client;
use brontes_database::database::Database;
use brontes_types::structured_trace::TxTrace;
use alloy_providers::provider::Provider;
use alloy_transport_http::Http;
use futures::Future;
use reth_interfaces::{provider::ProviderResult, RethError, RethResult};
use reth_primitives::{BlockId, BlockNumber, BlockNumberOrTag, Header, H160, H256};
use reth_provider::{BlockIdReader, BlockNumReader, HeaderProvider};
use reth_rpc_api::EthApiServer;
use reth_rpc_types::trace::parity::TraceType;
use reth_tracing_ext::TracingClient;
use tokio::{sync::mpsc::UnboundedSender, task::JoinError};

use self::parser::TraceParser;
use crate::{
    executor::{Executor, TaskKind},
    init_trace,
};

#[cfg(feature = "dyn-decode")]
mod dyn_decode;

pub mod parser;
mod utils;
pub mod vm_linker;
use brontes_metrics::{trace::types::TraceMetricEvent, PoirotMetricEvents};
#[allow(dead_code)]
pub(crate) const UNKNOWN: &str = "unknown";
#[allow(dead_code)]
pub(crate) const RECEIVE: &str = "receive";
#[allow(dead_code)]
pub(crate) const FALLBACK: &str = "fallback";

pub(crate) const CACHE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10_000);
pub(crate) const CACHE_DIRECTORY: &str = "./abi_cache";

use reth_rpc::eth::error::EthApiError;
use reth_rpc_types::{trace::parity::TraceResultsWithTransactionHash, TransactionReceipt};

#[async_trait::async_trait]
#[auto_impl::auto_impl(&, Arc, Box)]
pub trait TracingProvider: Send + Sync + 'static {
    async fn block_hash_for_id(&self, block_num: u64) -> ProviderResult<Option<H256>>;

    #[cfg(feature = "server")]
    fn best_block_number(&self) -> ProviderResult<u64>;

    #[cfg(not(feature = "server"))]
    async fn best_block_number(&self) -> ProviderResult<u64>;

    async fn replay_block_transactions(
        &self,
        block_id: BlockId,
        trace_type: HashSet<TraceType>,
    ) -> Result<Option<Vec<TraceResultsWithTransactionHash>>, EthApiError>;

    async fn block_receipts(
        &self,
        number: BlockNumberOrTag,
    ) -> ProviderResult<Option<Vec<TransactionReceipt>>>;

    async fn header_by_number(&self, number: BlockNumber) -> ProviderResult<Option<Header>>;
}

#[async_trait::async_trait]
impl TracingProvider for Provider<Http<Client>> {
    async fn block_hash_for_id(&self, block_num: u64) -> ProviderResult<Option<H256>> {
        todo!()
    }

    #[cfg(feature = "server")]
    fn best_block_number(&self) -> ProviderResult<u64> {
        todo!()
    }

    #[cfg(not(feature = "server"))]
    async fn best_block_number(&self) -> ProviderResult<u64> {
        todo!()
    }

    async fn replay_block_transactions(
        &self,
        block_id: BlockId,
        trace_type: HashSet<TraceType>,
    ) -> Result<Option<Vec<TraceResultsWithTransactionHash>>, EthApiError> {
        todo!()
    }

    async fn block_receipts(
        &self,
        number: BlockNumberOrTag,
    ) -> ProviderResult<Option<Vec<TransactionReceipt>>> {
        todo!()
    }

    async fn header_by_number(&self, number: BlockNumber) -> ProviderResult<Option<Header>> {
        todo!()
    }
}


#[async_trait::async_trait]
impl TracingProvider for TracingClient {
    async fn block_hash_for_id(&self, block_num: u64) -> ProviderResult<Option<H256>> {
        self.trace
            .provider()
            .block_hash_for_id(BlockId::Number(BlockNumberOrTag::Number(block_num.into())))
    }

    #[cfg(feature = "server")]
    fn best_block_number(&self) -> ProviderResult<u64> {
        self.trace.provider().best_block_number()
    }

    #[cfg(not(feature = "server"))]
    async fn best_block_number(&self) -> ProviderResult<u64> {
        self.trace.provider().best_block_number()
    }

    async fn replay_block_transactions(
        &self,
        block_id: BlockId,
        trace_type: HashSet<TraceType>,
    ) -> Result<Option<Vec<TraceResultsWithTransactionHash>>, EthApiError> {
        self.trace
            .replay_block_transactions(block_id, trace_type)
            .await
    }

    async fn block_receipts(
        &self,
        number: BlockNumberOrTag,
    ) -> ProviderResult<Option<Vec<TransactionReceipt>>> {
        Ok(Some(
            self.api
                .block_receipts(BlockId::Number(number))
                .await
                .unwrap()
                .unwrap(),
        ))
    }

    async fn header_by_number(&self, number: BlockNumber) -> ProviderResult<Option<Header>> {
        self.trace.provider().header_by_number(number)
    }
}

pub type ParserFuture<'a> =
    Pin<Box<dyn Future<Output = Result<Option<(Vec<TxTrace>, Header)>, JoinError>> + Send + 'a>>;

pub struct Parser<'a, T: TracingProvider> {
    executor: Executor,
    parser:   TraceParser<'a, T>,
}

impl<'a, T: TracingProvider> Parser<'a, T> {
    pub fn new(
        metrics_tx: UnboundedSender<PoirotMetricEvents>,
        database: &'a Database,
        tracing: T,
        should_fetch: Box<dyn Fn(&H160) -> bool + Send + Sync>,
    ) -> Self {
        let executor = Executor::new();

        let parser =
            TraceParser::new(database, should_fetch, Arc::new(tracing), Arc::new(metrics_tx));

        Self { executor, parser }
    }

    #[cfg(not(feature = "server"))]
    pub async fn get_latest_block_number(&self) -> ProviderResult<u64> {
        self.parser.tracer.best_block_number().await
    }

    #[cfg(feature = "server")]
    pub fn get_latest_block_number(&self) -> ProviderResult<u64> {
        self.parser.tracer.best_block_number()
    }

    pub async fn get_block_hash_for_number(&self, block_num: u64) -> ProviderResult<Option<H256>> {
        self.parser.tracer.block_hash_for_id(block_num.into()).await
    }

    /// executes the tracing of a given block
    pub fn execute(&self, block_num: u64) -> ParserFuture {
        // Safety: This is safe as the Arc ensures immutability.
        // This will satisfy its lifetime scope do to the lifetime itself living longer
        // than the process that runs brontes.
        let parser: &'static TraceParser<'static, T> = unsafe { std::mem::transmute(&self.parser) };

        Box::pin(
            self.executor
                .spawn_result_task_as(parser.execute_block(block_num), TaskKind::Default),
        ) as ParserFuture
    }
}
