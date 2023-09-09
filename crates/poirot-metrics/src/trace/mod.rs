pub mod types;
use metrics::{Counter, Gauge};
use reth_metrics::Metrics;
use std::collections::HashMap;
use tracing::trace;
pub mod utils;

use super::TraceMetricEvent;

#[derive(Debug, Default, Clone)]
pub struct TraceMetrics {
    pub(crate) txs: HashMap<String, TransactionTracingMetrics>,
}

impl TraceMetrics {
    /// Returns existing or initializes a new instance of [LiveRelayMetrics]
    pub(crate) fn get_transaction_metrics(
        &mut self,
        tx_hash: String,
    ) -> &mut TransactionTracingMetrics {
        self.txs.entry(tx_hash.clone()).or_insert_with(|| {
            TransactionTracingMetrics::new_with_labels(&[("transaction_tracing", tx_hash)])
        })
    }

    pub fn handle_event(&mut self, event: TraceMetricEvent) {
        trace!(target: "tracing::metrics", ?event, "Metric event received");
        match event {
            TraceMetricEvent::TraceMetricRecieved(_) => panic!("NOT IMPLEMENTED YET"),
            TraceMetricEvent::TransactionMetricRecieved(_) => panic!("NOT IMPLEMENTED YET"),
            TraceMetricEvent::BlockMetricRecieved(_) => panic!("NOT IMPLEMENTED YET"),
        }
    }
}

#[derive(Metrics, Clone)]
#[metrics(scope = "transaction_tracing")]
pub(crate) struct TransactionTracingMetrics {
    /// The block number currently on
    pub(crate) block_num: Gauge,
    /// The transaction index in the block
    pub(crate) tx_idx: Gauge,
    /// The trace index in the transaction
    pub(crate) tx_trace_idx: Gauge,
    /// The total amount of successful traces for this Transaction hash
    pub(crate) success_traces: Counter,
    /// The total amount of trace errors for this Transaction hash
    pub(crate) error_traces: Counter,
    /// Empty Input Errors
    pub(crate) empty_input_errors: Counter,
    /// Abi Parse Errors
    pub(crate) abi_parse_errors: Counter,
    /// Invalid Function Selector Errors
    pub(crate) invalid_function_selector_errors: Counter,
    /// Abi Decoding Failed Errors
    pub(crate) abi_decoding_failed_errors: Counter,
    /// Trace Missing Errors
    pub(crate) block_trace_missing_errors: Counter,
    /// Trace Missing Errors
    pub(crate) tx_trace_missing_errors: Counter,
    /// Etherscan Chain Not Supported
    pub(crate) etherscan_chain_not_supported: Counter,
    /// Etherscan Execution Failed
    pub(crate) etherscan_execution_failed: Counter,
    /// Etherscan Balance Failed
    pub(crate) etherscan_balance_failed: Counter,
    /// Etherscan Not Proxy
    pub(crate) etherscan_not_proxy: Counter,
    /// Etherscan Missing Implementation Address
    pub(crate) etherscan_missing_implementation_address: Counter,
    /// Etherscan Block Number By Timestamp Failed
    pub(crate) etherscan_block_number_by_timestamp_failed: Counter,
    /// Etherscan Transaction Receipt Failed
    pub(crate) etherscan_transaction_receipt_failed: Counter,
    /// Etherscan Gas Estimation Failed
    pub(crate) etherscan_gas_estimation_failed: Counter,
    /// Etherscan Bad Status Code
    pub(crate) etherscan_bad_status_code: Counter,
    /// Etherscan Env Var Not Found
    pub(crate) etherscan_env_var_not_found: Counter,
    /// Etherscan Reqwest
    pub(crate) etherscan_reqwest: Counter,
    /// Etherscan Serde
    pub(crate) etherscan_serde: Counter,
    /// Etherscan Contract Code Not Verified
    pub(crate) etherscan_contract_code_not_verified: Counter,
    /// Etherscan Empty Result
    pub(crate) etherscan_empty_result: Counter,
    /// Etherscan Rate Limit Exceeded
    pub(crate) etherscan_rate_limit_exceeded: Counter,
    /// Etherscan Io
    pub(crate) etherscan_io: Counter,
    /// Etherscan Local Networks Not Supported
    pub(crate) etherscan_local_networks_not_supported: Counter,
    /// Etherscan Error Response
    pub(crate) etherscan_error_response: Counter,
    /// Etherscan Unknown
    pub(crate) etherscan_unknown: Counter,
    /// Etherscan Builder Error
    pub(crate) etherscan_builder: Counter,
    /// Etherscan Missing Solc Version Error
    pub(crate) etherscan_missing_solc_version: Counter,
    /// Etherscan Invalid API Key Error
    pub(crate) etherscan_invalid_api_key: Counter,
    /// Etherscan Blocked By Cloudflare Error
    pub(crate) etherscan_blocked_by_cloudflare: Counter,
    /// Etherscan Cloudflair Security Challenge Error
    pub(crate) etherscan_cloudflare_security_challenge: Counter,
    /// Etherscan Page Not Found Error
    pub(crate) etherscan_page_not_found: Counter,
    /// Etherscan Cache Error
    pub(crate) etherscan_cache_error: Counter,
    /// Etherscan Cache Error
    pub(crate) eth_api_error: Counter,
    /// EthApi Empty Raw Transaction Data Errors
    pub(crate) eth_api_empty_raw_transaction_data: Counter,
    /// EthApi Failed To Decode Signed Transaction Errors
    pub(crate) eth_api_failed_to_decode_signed_transaction: Counter,
    /// EthApi Invalid Transaction Signature Errors
    pub(crate) eth_api_invalid_transaction_signature: Counter,
    /// EthApi Pool Error
    pub(crate) eth_api_pool_error: Counter,
    /// EthApi Unknown Block Number Errors
    pub(crate) eth_api_unknown_block_number: Counter,
    /// EthApi Unknown Block Or Tx Index Errors
    pub(crate) eth_api_unknown_block_or_tx_index: Counter,
    /// EthApi Invalid Block Range Errors
    pub(crate) eth_api_invalid_block_range: Counter,
    /// EthApi Prevrandao Not Set Errors
    pub(crate) eth_api_prevrandao_not_set: Counter,
    /// EthApi Conflicting Fee Fields In Request Errors
    pub(crate) eth_api_conflicting_fee_fields_in_request: Counter,
    /// EthApi Invalid Transaction Errors
    pub(crate) eth_api_invalid_transaction: Counter,
    /// EthApi Invalid Block Data Errors
    pub(crate) eth_api_invalid_block_data: Counter,
    /// EthApi Both State And State Diff In Override Errors
    pub(crate) eth_api_both_state_and_state_diff_in_override: Counter,
    /// EthApi Internal Errors
    pub(crate) eth_api_internal: Counter,
    /// EthApi Signing Errors
    pub(crate) eth_api_signing: Counter,
    /// EthApi Transaction Not Found Errors
    pub(crate) eth_api_transaction_not_found: Counter,
    /// EthApi Unsupported Errors
    pub(crate) eth_api_unsupported: Counter,
    /// EthApi Invalid Params Errors
    pub(crate) eth_api_invalid_params: Counter,
    /// EthApi Invalid Tracer Config Errors
    pub(crate) eth_api_invalid_tracer_config: Counter,
    /// EthApi Invalid Reward Percentiles Errors
    pub(crate) eth_api_invalid_reward_percentiles: Counter,
    /// EthApi Internal Tracing Error
    pub(crate) eth_api_internal_tracing_error: Counter,
    /// EthApi Internal Eth Error
    pub(crate) eth_api_internal_eth_error: Counter,
    /// EthApi Internal Js Tracer Error
    pub(crate) eth_api_internal_js_tracer_error: Counter,
}
