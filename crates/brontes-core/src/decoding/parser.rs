use std::sync::Arc;

use alloy_etherscan::Client;
use alloy_json_abi::JsonAbi;
use brontes_metrics::{
    trace::types::{BlockStats, TraceParseErrorKind, TraceStats, TransactionStats},
    PoirotMetricEvents,
};
use futures::future::join_all;
use reth_primitives::{Header, H256};
use reth_rpc_types::{
    trace::parity::{
        Action as RethAction, CallAction as RethCallAction, TraceResultsWithTransactionHash,
        TraceType, TransactionTrace, VmTrace,
    },
    Log, TransactionReceipt,
};

use super::*;
use crate::{decoding::vm_linker::link_vm_to_trace, errors::TraceParseError};

#[derive(Clone)]
/// A [`TraceParser`] will iterate through a block's Parity traces and attempt
/// to decode each call for later analysis.
pub(crate) struct TraceParser<T: TracingProvider> {
    etherscan_client:      Client,
    pub tracer:            Arc<T>,
    pub(crate) metrics_tx: Arc<UnboundedSender<PoirotMetricEvents>>,
}

impl<T: TracingProvider> TraceParser<T> {
    pub fn new(
        etherscan_client: Client,
        tracer: Arc<T>,
        metrics_tx: Arc<UnboundedSender<PoirotMetricEvents>>,
    ) -> Self {
        Self { etherscan_client, tracer, metrics_tx }
    }

    /// executes the tracing of a given block
    pub async fn execute_block(&self, block_num: u64) -> Option<(Vec<TxTrace>, Header)> {
        let parity_trace = self.trace_block(block_num).await;
        let receipts = self.get_receipts(block_num).await;

        if parity_trace.0.is_none() && receipts.0.is_none() {
            self.metrics_tx
                .send(TraceMetricEvent::BlockMetricRecieved(parity_trace.1).into())
                .unwrap();
            return None
        }
        let traces = self
            .parse_block(parity_trace.0.unwrap(), receipts.0.unwrap(), block_num)
            .await;
        self.metrics_tx
            .send(TraceMetricEvent::BlockMetricRecieved(traces.1).into())
            .unwrap();
        Some((traces.0, traces.2))
    }

    /// traces a block into a vec of tx traces
    pub(crate) async fn trace_block(
        &self,
        block_num: u64,
    ) -> (Option<Vec<TraceResultsWithTransactionHash>>, BlockStats) {
        let mut trace_type = HashSet::new();
        trace_type.insert(TraceType::Trace);
        trace_type.insert(TraceType::VmTrace);

        let parity_trace = self
            .tracer
            .replay_block_transactions(
                BlockId::Number(BlockNumberOrTag::Number(block_num)),
                trace_type,
            )
            .await;

        let mut stats = BlockStats::new(block_num, None);
        let trace = match parity_trace {
            Ok(Some(t)) => Some(t),
            Ok(None) => {
                stats.err = Some(TraceParseErrorKind::TracesMissingBlock);
                None
            }
            Err(e) => {
                stats.err = Some((&Into::<TraceParseError>::into(e)).into());
                None
            }
        };

        (trace, stats)
    }

    /// gets the transaction $receipts for a block
    pub(crate) async fn get_receipts(
        &self,
        block_num: u64,
    ) -> (Option<Vec<TransactionReceipt>>, BlockStats) {
        let tx_receipts = self
            .tracer
            .block_receipts(BlockNumberOrTag::Number(block_num))
            .await;
        let mut stats = BlockStats::new(block_num, None);

        let receipts = match tx_receipts {
            Ok(Some(t)) => Some(t),
            Ok(None) => {
                stats.err = Some(TraceParseErrorKind::TracesMissingBlock);
                None
            }
            _ => None,
        };

        (receipts, stats)
    }

    pub(crate) async fn parse_block(
        &self,
        block_trace: Vec<TraceResultsWithTransactionHash>,
        block_receipts: Vec<TransactionReceipt>,
        block_num: u64,
    ) -> (Vec<TxTrace>, BlockStats, Header) {
        let mut stats = BlockStats::new(block_num, None);

        let (traces, tx_stats): (Vec<_>, Vec<_>) =
            join_all(block_trace.into_iter().zip(block_receipts.into_iter()).map(
                |(trace, receipt)| async move {
                    let transaction_traces = trace.full_trace.trace;
                    let vm_traces = trace.full_trace.vm_trace.unwrap();

                    let tx_hash = trace.transaction_hash;

                    self.parse_transaction(
                        transaction_traces,
                        vm_traces,
                        receipt.logs,
                        block_num,
                        tx_hash,
                        receipt.transaction_index.try_into().unwrap(),
                        receipt.gas_used.unwrap().to(),
                        receipt.effective_gas_price.to(),
                    )
                    .await
                },
            ))
            .await
            .into_iter()
            .unzip();

        stats.txs = tx_stats;
        stats.trace();

        (
            traces,
            stats,
            self.tracer
                .header_by_number(block_num)
                .await
                .unwrap()
                .unwrap(),
        )
    }

    /// parses a transaction and gathers the traces
    async fn parse_transaction(
        &self,
        tx_trace: Vec<TransactionTrace>,
        vm: VmTrace,
        logs: Vec<Log>,
        block_num: u64,
        tx_hash: H256,
        tx_idx: u64,
        gas_used: u64,
        effective_gas_price: u64,
    ) -> (TxTrace, TransactionStats) {
        init_trace!(tx_hash, tx_idx, tx_trace.len());
        let mut traces = Vec::new();
        let mut stats = TransactionStats {
            block_num,
            tx_hash,
            tx_idx: tx_idx as u16,
            traces: vec![],
            err: None,
        };

        let len = tx_trace.len();

        println!("{:?}", tx_hash);
        let linked_trace = link_vm_to_trace(vm, tx_trace, logs);

        for (idx, trace) in linked_trace.into_iter().enumerate() {
            let abi_trace = self
                .update_abi_cache(trace.trace.clone(), block_num, tx_hash)
                .await;
            let mut stat = TraceStats::new(block_num, tx_hash, tx_idx as u16, idx as u16, None);
            if let Err(e) = abi_trace {
                stat.err = Some(Into::<TraceParseErrorKind>::into(&e));
            } else {
                traces.push(trace);
            }
            stat.trace(len);
            stats.traces.push(stat);
        }

        stats.trace();
        (TxTrace::new(traces, tx_hash, tx_idx, gas_used, effective_gas_price), stats)
    }

    /// pushes each trace to parser_fut
    async fn update_abi_cache(
        &self,
        trace: TransactionTrace,
        block_num: u64,
        tx_hash: H256,
    ) -> Result<(), TraceParseError> {
        let (action, trace_address) = if let RethAction::Call(call) = trace.action {
            (call, trace.trace_address)
        } else {
            return Ok(())
        };

        //let binding = StaticBindings::Curve_Crypto_Factory_V2;
        let _addr = format!("{:#x}", action.from);
        let abi = //if let Some(abi_path) = PROTOCOL_ADDRESS_MAPPING.get(&addr) {
            //serde_json::from_str(abi_path).map_err(|e| TraceParseError::AbiParseError(e))?
        //} else {
            self.etherscan_client.contract_abi(action.to.into()).await?;
        //};

        // Check if the input is empty, indicating a potential `receive` or `fallback`
        // function call.
        if action.input.is_empty() {
            return Ok(())
        }

        let _ = self
            .abi_decoding_pipeline(&abi, &action, &trace_address, &tx_hash, block_num)
            .await;
        Ok(())
    }

    /// cycles through all possible abi decodings
    /// 1) regular
    /// 2) proxy
    /// 3) diamond proxy
    async fn abi_decoding_pipeline(
        &self,
        _abi: &JsonAbi,
        action: &RethCallAction,
        _trace_address: &[usize],
        _tx_hash: &H256,
        _block_num: u64,
    ) -> Result<(), TraceParseError> {
        // check decoding with the regular abi

        // tries to get the proxy abi -> decode
        let _proxy_abi = self
            .etherscan_client
            .proxy_contract_abi(action.to.into())
            .await?;

        Ok(())
    }
}

pub(crate) mod test_utils {
    use std::{
        env,
        path::{Path, PathBuf},
        sync::Arc,
    };

    use alloy_etherscan::Client;
    use brontes_metrics::PoirotMetricEvents;
    use ethers_core::types::Chain;
    use reth_blockchain_tree::metrics;
    use reth_tracing::TracingClient;
    use tokio::{
        runtime::Handle,
        sync::mpsc::{unbounded_channel, UnboundedSender},
    };

    use super::TraceParser;
    use crate::decoding::{CACHE_DIRECTORY, CACHE_TIMEOUT};

    pub(crate) fn init_trace_parser(
        handle: Handle,
        metrics_tx: UnboundedSender<PoirotMetricEvents>,
    ) -> TraceParser<TracingClient> {
        let etherscan_key = env::var("ETHERSCAN_API_KEY").expect("No ETHERSCAN_API_KEY in .env");
        let db_path = env::var("DB_PATH").expect("No DB_PATH in .env");

        let etherscan_client = Client::new_cached(
            Chain::Mainnet,
            etherscan_key,
            Some(PathBuf::from(CACHE_DIRECTORY)),
            CACHE_TIMEOUT,
        )
        .unwrap();

        let tracer = TracingClient::new(Path::new(&db_path), handle.clone());

        TraceParser::new(etherscan_client, Arc::new(tracer), Arc::new(metrics_tx))
    }
}
