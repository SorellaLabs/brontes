use metrics::{Gauge, Counter};
use reth_metrics::Metrics;
use std::collections::HashMap;

#[derive(Debug, Default, Clone)]
pub(crate) struct TraceMetrics {
    pub(crate) txs: HashMap<String, TransactionTracingMetrics>,
}

impl TraceMetrics {
    /// Returns existing or initializes a new instance of [LiveRelayMetrics]
    pub(crate) fn get_transaction_metrics(&mut self, tx_hash: String) -> &mut TransactionTracingMetrics {
        self.txs
            .entry(tx_hash.clone())
            .or_insert_with(|| TransactionTracingMetrics::new_with_labels(&[("Transaction_tracing", tx_hash)]))
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
    pub(crate) trace_missing_errors: Counter,
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
}
