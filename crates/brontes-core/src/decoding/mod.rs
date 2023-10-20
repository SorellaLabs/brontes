use std::{collections::HashSet, path::PathBuf, pin::Pin, sync::Arc};

use alloy_etherscan::Client;
use brontes_types::structured_trace::TxTrace;
use ethers::prelude::{Http, Middleware, Provider};
use ethers_core::types::Chain;
use ethers_reth::type_conversions::{ToEthers, ToReth};
use futures::Future;
use reth_interfaces::RethError;
use reth_primitives::{BlockId, BlockNumber, BlockNumberOrTag, Header, H256};
use reth_provider::{BlockIdReader, BlockNumReader, HeaderProvider};
use reth_rpc_api::EthApiServer;
use reth_rpc_types::trace::parity::TraceType;
use reth_tracing::TracingClient;
use tokio::{sync::mpsc::UnboundedSender, task::JoinError};

use self::parser::TraceParser;
use crate::{
    executor::{Executor, TaskKind},
    init_trace,
};

pub(crate) mod parser;
mod utils;
pub(crate) mod vm_linker;
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
#[auto_impl::auto_impl(&, Arc)]
pub trait TracingProvider: Send + Sync + 'static {
    async fn block_hash_for_id(&self, block_num: u64) -> reth_interfaces::RethResult<Option<H256>>;

    #[cfg(feature = "server")]
    fn best_block_number(&self) -> reth_interfaces::RethResult<u64>;

    #[cfg(not(feature = "server"))]
    async fn best_block_number(&self) -> reth_interfaces::RethResult<u64>;

    async fn replay_block_transactions(
        &self,
        block_id: BlockId,
        trace_type: HashSet<TraceType>,
    ) -> Result<Option<Vec<TraceResultsWithTransactionHash>>, EthApiError>;

    async fn block_receipts(
        &self,
        number: BlockNumberOrTag,
    ) -> reth_interfaces::RethResult<Option<Vec<TransactionReceipt>>>;

    async fn header_by_number(
        &self,
        number: BlockNumber,
    ) -> reth_interfaces::RethResult<Option<Header>>;
}

#[async_trait::async_trait]
impl TracingProvider for Provider<Http> {
    async fn block_hash_for_id(&self, block_num: u64) -> reth_interfaces::RethResult<Option<H256>> {
        self.get_block(block_num)
            .await
            .map(|h| h.map(|e| e.into_reth().inner.header.hash.take().unwrap()))
            .map_err(|e| RethError::Custom(format!("{}", e)))
    }

    #[cfg(not(feature = "server"))]
    async fn best_block_number(&self) -> reth_interfaces::RethResult<u64> {
        self.get_block_number()
            .await
            .map(|r| r.as_u64())
            .map_err(|e| RethError::Custom(format!("{}", e)))
    }

    #[cfg(feature = "server")]
    fn best_block_number(&self) -> reth_interfaces::RethResult<u64> {
        unreachable!()
    }

    async fn replay_block_transactions(
        &self,
        block_id: BlockId,
        trace_type: HashSet<TraceType>,
    ) -> Result<Option<Vec<TraceResultsWithTransactionHash>>, EthApiError> {
        let block_id = match block_id {
            BlockId::Number(t) => t.as_number().unwrap(),
            _ => return Err(EthApiError::PrevrandaoNotSet),
        };
        Ok(Some(
            self.trace_replay_block_transactions(
                block_id.into(),
                trace_type
                    .into_iter()
                    .map(|i| i.into_ethers())
                    .collect::<Vec<_>>(),
            )
            .await
            .unwrap()
            .into_iter()
            .map(|m| m.into_reth())
            .collect::<Vec<_>>(),
        ))
    }

    async fn block_receipts(
        &self,
        number: BlockNumberOrTag,
    ) -> reth_interfaces::RethResult<Option<Vec<TransactionReceipt>>> {
        self.get_block_receipts(number.as_number().unwrap())
            .await
            .map(|receipts| {
                Some(
                    receipts
                        .into_iter()
                        .map(|t| t.into_reth())
                        .collect::<Vec<TransactionReceipt>>(),
                )
            })
            .map_err(|e| RethError::Custom(format!("{}", e)))
    }

    async fn header_by_number(
        &self,
        number: BlockNumber,
    ) -> reth_interfaces::RethResult<Option<Header>> {
        self.get_block(number)
            .await
            .map(|opt_block| {
                opt_block.map(|a| {
                    let mut header = Header::default();
                    header.base_fee_per_gas = a.base_fee_per_gas.map(|f| f.as_u64());
                    header
                })
            })
            .map_err(|e| RethError::Custom(format!("{}", e)))
    }
}

#[async_trait::async_trait]
impl TracingProvider for TracingClient {
    async fn block_hash_for_id(&self, block_num: u64) -> reth_interfaces::RethResult<Option<H256>> {
        self.trace
            .provider()
            .block_hash_for_id(BlockId::Number(BlockNumberOrTag::Number(block_num)))
    }

    #[cfg(feature = "server")]
    fn best_block_number(&self) -> reth_interfaces::RethResult<u64> {
        self.trace.provider().best_block_number()
    }

    #[cfg(not(feature = "server"))]
    async fn best_block_number(&self) -> reth_interfaces::RethResult<u64> {
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
    ) -> reth_interfaces::RethResult<Option<Vec<TransactionReceipt>>> {
        Ok(Some(self.api.block_receipts(number).await.unwrap().unwrap()))
    }

    async fn header_by_number(
        &self,
        number: BlockNumber,
    ) -> reth_interfaces::RethResult<Option<Header>> {
        self.trace.provider().header_by_number(number)
    }
}

pub type ParserFuture = Pin<
    Box<dyn Future<Output = Result<Option<(Vec<TxTrace>, Header)>, JoinError>> + Send + 'static>,
>;

pub struct Parser<T: TracingProvider> {
    executor: Executor,
    parser:   Arc<TraceParser<T>>,
}

impl<T: TracingProvider> Parser<T> {
    pub fn new(
        metrics_tx: UnboundedSender<PoirotMetricEvents>,
        etherscan_key: &str,
        tracing: T,
    ) -> Self {
        let executor = Executor::new();
        // let tracer =
        //     Arc::new(TracingClient::new(Path::new(db_path),
        // executor.runtime.handle().clone()));

        let etherscan_client = Client::new_cached(
            Chain::Mainnet,
            etherscan_key,
            Some(PathBuf::from(CACHE_DIRECTORY)),
            CACHE_TIMEOUT,
        )
        .unwrap();
        let parser = TraceParser::new(etherscan_client, Arc::new(tracing), Arc::new(metrics_tx));

        Self { executor, parser: Arc::new(parser) }
    }

    #[cfg(not(feature = "server"))]
    pub async fn get_latest_block_number(&self) -> reth_interfaces::RethResult<u64> {
        self.parser.tracer.best_block_number().await
    }

    #[cfg(feature = "server")]
    pub fn get_latest_block_number(&self) -> reth_interfaces::RethResult<u64> {
        self.parser.tracer.best_block_number()
    }

    pub async fn get_block_hash_for_number(
        &self,
        block_num: u64,
    ) -> reth_interfaces::RethResult<Option<H256>> {
        self.parser.tracer.block_hash_for_id(block_num.into()).await
    }

    /// executes the tracing of a given block
    pub fn execute(&self, block_num: u64) -> ParserFuture {
        let parser = self.parser.clone();
        Box::pin(self.executor.spawn_result_task_as(
            async move { parser.execute_block(block_num).await },
            TaskKind::Default,
        )) as ParserFuture
    }
}
