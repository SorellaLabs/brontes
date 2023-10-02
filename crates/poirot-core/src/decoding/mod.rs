use std::{
    any::Any,
    collections::HashSet,
    path::{Path, PathBuf},
    pin::Pin,
    sync::Arc,
};

use alloy_etherscan::Client;
use ethers::prelude::{Middleware, Provider};
use ethers_core::types::Chain;
use ethers_reth::type_conversions::{ToEthers, ToReth};
use futures::Future;
use poirot_types::structured_trace::TxTrace;
use reth_interfaces::{RethError, RethResult};
use reth_primitives::{BlockId, BlockNumber, BlockNumberOrTag, Header, H256};
use reth_provider::{BlockIdReader, BlockNumReader};
use reth_rpc_types::trace::parity::TraceType;
use reth_tracing::TracingClient;
use tokio::{sync::mpsc::UnboundedSender, task::JoinError};

use self::parser::TraceParser;
use crate::{
    executor::{Executor, TaskKind},
    init_trace,
};

mod parser;
mod utils;
use poirot_metrics::{trace::types::TraceMetricEvent, PoirotMetricEvents};
#[allow(dead_code)]
pub(crate) const UNKNOWN: &str = "unknown";
#[allow(dead_code)]
pub(crate) const RECEIVE: &str = "receive";
#[allow(dead_code)]
pub(crate) const FALLBACK: &str = "fallback";

const CACHE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10_000);
const CACHE_DIRECTORY: &str = "./abi_cache";

use reth_rpc::eth::error::EthApiError;
use reth_rpc_types::{
    trace::parity::{
        Action as RethAction, CallAction as RethCallAction, TraceResultsWithTransactionHash,
        TransactionTrace,
    },
    Log, TransactionReceipt,
};

#[async_trait::async_trait]
pub trait TracingProvider: Send + Sync + 'static {
    async fn block_hash_for_id(&self, block_num: u64) -> reth_interfaces::RethResult<Option<H256>>;

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
impl<T: Middleware + 'static> TracingProvider for T
where
    RethError: From<<T as Middleware>::Error>,
    EthApiError: From<<T as Middleware>::Error>,
{
    async fn block_hash_for_id(&self, block_num: u64) -> reth_interfaces::RethResult<Option<H256>> {
        Ok(self
            .get_block(block_num)
            .await?
            .map(|h| h.hash.map(|e| e.into()))
            .flatten())
    }

    async fn best_block_number(&self) -> reth_interfaces::RethResult<u64> {
        self.get_block_number()
            .await
            .map(|r| r.as_u64())
            .map_err(|e| e.into())
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
            .await?
            .into_iter()
            .map(|m| m.into_reth())
            .collect::<Vec<_>>(),
        ))
    }

    async fn block_receipts(
        &self,
        number: BlockNumberOrTag,
    ) -> reth_interfaces::RethResult<Option<Vec<TransactionReceipt>>> {
        let number = number.as_number().unwrap();

        Ok(Some(
            self.get_block_receipts(number)
                .await?
                .into_iter()
                .map(|t| t.into_reth())
                .collect::<Vec<TransactionReceipt>>(),
        ))
    }

    async fn header_by_number(
        &self,
        number: BlockNumber,
    ) -> reth_interfaces::RethResult<Option<Header>> {
        todo!()
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
        db_path: &str,
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

    pub async fn get_block_hash_for_number(
        &self,
        block_num: u64,
    ) -> reth_interfaces::RethResult<Option<H256>> {
        self.parser.tracer.block_hash_for_id(block_num.into()).await
    }

    pub async fn get_latest_block_number(&self) -> RethResult<BlockNumber> {
        self.parser.tracer.best_block_number().await
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
