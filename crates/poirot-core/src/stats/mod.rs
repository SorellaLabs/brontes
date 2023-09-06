pub mod macros;
pub mod metrics;
pub(crate) mod types;

use reth_primitives::H256;
use revm_primitives::B256;
use std::{
    future::Future,
    pin::Pin,
    task::{ready, Context, Poll},
};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tracing::trace;

use crate::errors::TraceParseErrorKind;

use self::{
    metrics::{TraceMetrics, TransactionTracingMetrics},
    types::*,
};

/// Alias type for metric producers to use.
pub type TraceMetricEventsSender = UnboundedSender<TraceMetricEvent>;

/// metric event for traces
#[derive(Clone, Debug)]
pub enum TraceMetricEvent {
    /// recorded a new block trace
    BlockMetricRecieved(BlockStats),
    /// recorded a new tx trace
    TransactionMetricRecieved(TransactionStats),
    /// recorded a new individual tx trace
    TraceMetricRecieved(TraceStats),
}

/// Metrics routine that listens to new metric events on the `events_rx` receiver.
/// Upon receiving new event, related metrics are updated.
#[derive(Debug)]
pub struct TraceMetricsListener {
    events_rx: UnboundedReceiver<TraceMetricEvent>,
    pub(crate) tx_metrics: TraceMetrics,
}

impl TraceMetricsListener {
    /// Creates a new [MetricsListener] with the provided receiver of [MetricEvent].
    pub fn new(events_rx: UnboundedReceiver<TraceMetricEvent>) -> Self {
        Self { events_rx, tx_metrics: TraceMetrics::default() }
    }

    fn handle_event(&mut self, event: TraceMetricEvent) {
        trace!(target: "tracing::metrics", ?event, "Metric event received");
        match event {
            TraceMetricEvent::TraceMetricRecieved(_) => (), //TO IMPLEMENT
            TraceMetricEvent::TransactionMetricRecieved(_) => panic!("NOT IMPLEMENTED YET"),
            TraceMetricEvent::BlockMetricRecieved(_) => panic!("NOT IMPLEMENTED YET"),
            /*
            TraceMetricEvent::TraceMetricRecieved {
                block_num,
                tx_hash,
                tx_idx,
                tx_trace_idx,
                error,
            } => {
                let tx_metrics = self.tx_metrics.get_transaction_metrics(format!("{:#x}", tx_hash));

                tx_metrics.block_num.set(block_num as f64);
                tx_metrics.tx_idx.set(tx_idx as f64);
                tx_metrics.tx_trace_idx.set(tx_trace_idx as f64);

                if let Some(err) = error {
                    tx_metrics.error_traces.increment(1);
                    increment_error(tx_metrics, err);
                } else {
                    tx_metrics.success_traces.increment(1);
                }
            }
            TraceMetricEvent::TransactionMetricRecieved { block_num, tx_hash, tx_idx, error } => (), //todo
            TraceMetricEvent::BlockTracingErrorMetric { block_num, error } => (),
            TraceMetricEvent::TxTracingErrorMetric { block_num, tx_hash, tx_idx, error } => {
                let tx_metrics = self.tx_metrics.get_transaction_metrics(format!("{:#x}", tx_hash));

                tx_metrics.block_num.set(block_num as f64);
                tx_metrics.tx_idx.set(tx_idx as f64);
                tx_metrics.tx_trace_idx.set(tx_idx as f64);

                tx_metrics.error_traces.increment(1);
                increment_error(tx_metrics, error);
            }
            TraceMetricEvent::BlockMetricRecieved { block_num, error } => (),
             */
        }
    }
}

impl Future for TraceMetricsListener {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();

        // Loop until we drain the `events_rx` channel
        // TODO: use while let
        loop {
            let Some(event) = ready!(this.events_rx.poll_recv(cx)) else {
                // Channel has closed
                return Poll::Ready(());
            };

            this.handle_event(event);
        }
    }
}

/// TODO: I would of just made a qick macro here todo this automatically because its a
///  1-1 defined mapping and im lazy and dont trust chatgpt. also kinda autistic how these don't
///  match
/// computes error increment
fn increment_error(tx_metric: &mut TransactionTracingMetrics, error: TraceParseErrorKind) {
    match error {
        TraceParseErrorKind::TracesMissingBlock => {
            tx_metric.block_trace_missing_errors.increment(1)
        }
        TraceParseErrorKind::TracesMissingTx => tx_metric.tx_trace_missing_errors.increment(1),
        TraceParseErrorKind::EthApiError => tx_metric.eth_api_error.increment(1),
        TraceParseErrorKind::EmptyInput => tx_metric.empty_input_errors.increment(1),
        TraceParseErrorKind::AbiParseError => tx_metric.abi_parse_errors.increment(1),
        TraceParseErrorKind::InvalidFunctionSelector => {
            tx_metric.invalid_function_selector_errors.increment(1)
        }
        TraceParseErrorKind::AbiDecodingFailed => tx_metric.abi_decoding_failed_errors.increment(1),
        TraceParseErrorKind::EtherscanChainNotSupported => {
            tx_metric.etherscan_chain_not_supported.increment(1)
        }
        TraceParseErrorKind::EtherscanExecutionFailed => {
            tx_metric.etherscan_execution_failed.increment(1)
        }
        TraceParseErrorKind::EtherscanBalanceFailed => {
            tx_metric.etherscan_balance_failed.increment(1)
        }
        TraceParseErrorKind::EtherscanNotProxy => tx_metric.etherscan_not_proxy.increment(1),
        TraceParseErrorKind::EtherscanMissingImplementationAddress => {
            tx_metric.etherscan_missing_implementation_address.increment(1)
        }
        TraceParseErrorKind::EtherscanBlockNumberByTimestampFailed => {
            tx_metric.etherscan_block_number_by_timestamp_failed.increment(1)
        }
        TraceParseErrorKind::EtherscanTransactionReceiptFailed => {
            tx_metric.etherscan_transaction_receipt_failed.increment(1)
        }
        TraceParseErrorKind::EtherscanGasEstimationFailed => {
            tx_metric.etherscan_gas_estimation_failed.increment(1)
        }
        TraceParseErrorKind::EtherscanBadStatusCode => {
            tx_metric.etherscan_bad_status_code.increment(1)
        }
        TraceParseErrorKind::EtherscanEnvVarNotFound => {
            tx_metric.etherscan_env_var_not_found.increment(1)
        }
        TraceParseErrorKind::EtherscanReqwest => tx_metric.etherscan_reqwest.increment(1),
        TraceParseErrorKind::EtherscanSerde => tx_metric.etherscan_serde.increment(1),
        TraceParseErrorKind::EtherscanContractCodeNotVerified => {
            tx_metric.etherscan_contract_code_not_verified.increment(1)
        }
        TraceParseErrorKind::EtherscanEmptyResult => tx_metric.etherscan_empty_result.increment(1),
        TraceParseErrorKind::EtherscanRateLimitExceeded => {
            tx_metric.etherscan_rate_limit_exceeded.increment(1)
        }
        TraceParseErrorKind::EtherscanIO => tx_metric.etherscan_io.increment(1),
        TraceParseErrorKind::EtherscanLocalNetworksNotSupported => {
            tx_metric.etherscan_local_networks_not_supported.increment(1)
        }
        TraceParseErrorKind::EtherscanErrorResponse => {
            tx_metric.etherscan_error_response.increment(1)
        }
        TraceParseErrorKind::EtherscanUnknown => tx_metric.etherscan_unknown.increment(1),
        TraceParseErrorKind::EtherscanBuilder => tx_metric.etherscan_builder.increment(1),
        TraceParseErrorKind::EtherscanMissingSolcVersion => {
            tx_metric.etherscan_missing_solc_version.increment(1)
        }
        TraceParseErrorKind::EtherscanInvalidApiKey => {
            tx_metric.etherscan_invalid_api_key.increment(1)
        }
        TraceParseErrorKind::EtherscanBlockedByCloudflare => {
            tx_metric.etherscan_blocked_by_cloudflare.increment(1)
        }
        TraceParseErrorKind::EtherscanCloudFlareSecurityChallenge => {
            tx_metric.etherscan_cloudflare_security_challenge.increment(1)
        }
        TraceParseErrorKind::EtherscanPageNotFound => {
            tx_metric.etherscan_page_not_found.increment(1)
        }
        TraceParseErrorKind::EtherscanCacheError => tx_metric.etherscan_cache_error.increment(1),
        TraceParseErrorKind::ChannelSendError => (),
        TraceParseErrorKind::EthApiEmptyRawTransactionData => {
            tx_metric.eth_api_empty_raw_transaction_data.increment(1)
        }
        TraceParseErrorKind::EthApiFailedToDecodeSignedTransaction => {
            tx_metric.eth_api_failed_to_decode_signed_transaction.increment(1)
        }
        TraceParseErrorKind::EthApiInvalidTransactionSignature => {
            tx_metric.eth_api_invalid_transaction_signature.increment(1)
        }
        TraceParseErrorKind::EthApiPoolError => tx_metric.eth_api_pool_error.increment(1),
        TraceParseErrorKind::EthApiUnknownBlockNumber => {
            tx_metric.eth_api_unknown_block_number.increment(1)
        }
        TraceParseErrorKind::EthApiUnknownBlockOrTxIndex => {
            tx_metric.eth_api_unknown_block_or_tx_index.increment(1)
        }
        TraceParseErrorKind::EthApiInvalidBlockRange => {
            tx_metric.eth_api_invalid_block_range.increment(1)
        }
        TraceParseErrorKind::EthApiPrevrandaoNotSet => {
            tx_metric.eth_api_prevrandao_not_set.increment(1)
        }
        TraceParseErrorKind::EthApiConflictingFeeFieldsInRequest => {
            tx_metric.eth_api_conflicting_fee_fields_in_request.increment(1)
        }
        TraceParseErrorKind::EthApiInvalidTransaction => {
            tx_metric.eth_api_invalid_transaction.increment(1)
        }
        TraceParseErrorKind::EthApiInvalidBlockData => {
            tx_metric.eth_api_invalid_block_data.increment(1)
        }
        TraceParseErrorKind::EthApiBothStateAndStateDiffInOverride => {
            tx_metric.eth_api_both_state_and_state_diff_in_override.increment(1)
        }
        TraceParseErrorKind::EthApiInternal => tx_metric.eth_api_internal.increment(1),
        TraceParseErrorKind::EthApiSigning => tx_metric.eth_api_signing.increment(1),
        TraceParseErrorKind::EthApiTransactionNotFound => {
            tx_metric.eth_api_transaction_not_found.increment(1)
        }
        TraceParseErrorKind::EthApiUnsupported => tx_metric.eth_api_unsupported.increment(1),
        TraceParseErrorKind::EthApiInvalidParams => tx_metric.eth_api_invalid_params.increment(1),
        TraceParseErrorKind::EthApiInvalidTracerConfig => {
            tx_metric.eth_api_invalid_tracer_config.increment(1)
        }
        TraceParseErrorKind::EthApiInvalidRewardPercentiles => {
            tx_metric.eth_api_invalid_reward_percentiles.increment(1)
        }
        TraceParseErrorKind::EthApiInternalTracingError => {
            tx_metric.eth_api_internal_tracing_error.increment(1)
        }
        TraceParseErrorKind::EthApiInternalEthError => {
            tx_metric.eth_api_internal_eth_error.increment(1)
        }
        TraceParseErrorKind::EthApiInternalJsTracerError => {
            tx_metric.eth_api_internal_js_tracer_error.increment(1)
        }
    }
}
