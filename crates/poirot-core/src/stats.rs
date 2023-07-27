use std::sync::atomic::Ordering;

use crate::{format_color, errors::TraceParseError, *};
use alloy_etherscan::errors::EtherscanError;
use tracing::{
    field::{Field, Visit},
    span::Attributes,
    Id, Subscriber, info, Level,
};
use tracing_subscriber::{layer::Context, registry::LookupSpan, Layer, EnvFilter, FmtSubscriber};

pub struct ParserStatsLayer;


#[derive(Debug, Default)]
pub struct EtherscanErrorStats {
    pub total: usize,
    pub chain_not_supported: usize,
    pub execution_failed: usize,
    pub balance_failed: usize,
    pub not_proxy: usize,
    pub missing_implementation_address: usize,
    pub block_number_by_timestamp_failed: usize,
    pub transaction_receipt_failed: usize,
    pub gas_estimation_failed: usize,
    pub bad_status_code: usize,
    pub env_var_not_found: usize,
    pub reqwest: usize,
    pub serde: usize,
    pub contract_code_not_verified: usize,
    pub empty_result: usize,
    pub rate_limit_exceeded: usize,
    pub io: usize,
    pub local_networks_not_supported: usize,
    pub error_response: usize,
    pub unknown: usize,
    pub builder: usize,
    pub missing_solc_version: usize,
    pub invalid_api_key: usize,
    pub blocked_by_cloudflare: usize,
    pub cloudflare_security_challenge: usize,
    pub page_not_found: usize,
    pub cache_error: usize,
}

impl EtherscanErrorStats {
    pub fn into_info(&self) {
        /* 
        if self.total > 0 {
            info!(" {}", format_color("Etherscan Errors", self.total, true));
        }
        */
        if self.chain_not_supported > 0 {
            info!("{}", format_color("Etherscan Error -- Chain Not Supported", self.chain_not_supported, true));
        }
        if self.execution_failed > 0 {
            info!("{}", format_color("Etherscan Error -- Execution Failed", self.execution_failed, true));
        }
        if self.balance_failed > 0 {
            info!("{}", format_color("Etherscan Error -- Balance Failed", self.balance_failed, true));
        }
        if self.not_proxy > 0 {
            info!("{}", format_color("Etherscan Error -- Not a Proxy Contract", self.not_proxy, true));
        }
        if self.missing_implementation_address > 0 {
            info!("{}", format_color("Etherscan Error -- Missing Implementation Address", self.missing_implementation_address, true));
        }
        if self.block_number_by_timestamp_failed > 0 {
            info!("{}", format_color("Etherscan Error -- Block by Timestamp Failed", self.block_number_by_timestamp_failed, true));
        }
        if self.transaction_receipt_failed > 0 {
            info!("{}", format_color("Etherscan Error -- Transaction Receipt Failed", self.transaction_receipt_failed, true));
        }
        if self.gas_estimation_failed > 0 {
            info!("{}", format_color("Etherscan Error -- Gas Estimation Failed", self.gas_estimation_failed, true));
        }
        if self.bad_status_code > 0 {
            info!("{}", format_color("Etherscan Error -- Bad Status Code", self.bad_status_code, true));
        }
        if self.env_var_not_found > 0 {
            info!("{}", format_color("Etherscan Error -- Environment Variable Not Found", self.env_var_not_found, true));
        }
        if self.reqwest > 0 {
            info!("{}", format_color("Etherscan Error -- Reqwest Error", self.reqwest, true));
        }
        if self.serde > 0 {
            info!("{}", format_color("Etherscan Error -- Serde Error", self.serde, true));
        }
        if self.contract_code_not_verified > 0 {
            info!("{}", format_color("Etherscan Error -- Contract Code Not Verified", self.contract_code_not_verified, true));
        }
        if self.empty_result > 0 {
            info!("{}", format_color("Etherscan Error -- Empty Result", self.empty_result, true));
        }
        if self.rate_limit_exceeded > 0 {
            info!("{}", format_color("Etherscan Error -- Rate Limit Exceeded", self.rate_limit_exceeded, true));
        }
        if self.io > 0 {
            info!("{}", format_color("Etherscan Error -- IO Error", self.io, true));
        }
        if self.local_networks_not_supported > 0 {
            info!("{}", format_color("Etherscan Error -- Local Networks Not Supported", self.local_networks_not_supported, true));
        }
        if self.error_response > 0 {
            info!("{}", format_color("Etherscan Error -- Error Response", self.error_response, true));
        }
        if self.unknown > 0 {
            info!("{}", format_color("Etherscan Error -- Unknown Error", self.unknown, true));
        }
        if self.builder > 0 {
            info!("{}", format_color("Etherscan Error -- Builder Error", self.builder, true));
        }
        if self.missing_solc_version > 0 {
            info!("{}", format_color("Etherscan Error -- Missing Solc Version", self.missing_solc_version, true));
        }
        if self.invalid_api_key > 0 {
            info!("{}", format_color("Etherscan Error -- Invalid API Key", self.invalid_api_key, true));
        }
        if self.blocked_by_cloudflare > 0 {
            info!("{}", format_color("Etherscan Error -- Blocked By Cloudflare", self.blocked_by_cloudflare, true));
        }
        if self.cloudflare_security_challenge > 0 {
            info!("{}", format_color("Etherscan Error -- Cloudflare Security Challenge", self.cloudflare_security_challenge, true));
        }
        if self.page_not_found > 0 {
            info!("{}", format_color("Etherscan Error -- Page Not Found", self.page_not_found, true));
        }
        if self.cache_error > 0 {
            info!("{}", format_color("Etherscan Error -- Cache Error", self.cache_error, true));
        }
    }
}


#[derive(Debug, Default)]
pub struct ParserErrorStats {
    //pub total_txs: usize,
    //pub total_traces: usize,
    //pub successful_parses: usize,
    pub empty_input_errors: usize,
    pub etherscan_errors: EtherscanErrorStats,
    pub abi_parse_errors: usize,
    pub invalid_function_selector_errors: usize,
    pub abi_decoding_failed_errors: usize,
    pub trace_missing_errors: usize,
}

impl ParserErrorStats {
    /// Since we are calling this from another layer that doesn't implement outputing to stdout
    /// We can initiate a fmt layer to output the stats as such
    pub fn print_stats(&self) {
        tracing::subscriber::with_default(
            FmtSubscriber::builder()
                .with_env_filter(EnvFilter::builder().with_default_directive(Level::INFO.into()).from_env_lossy())
                .finish(), 
            || {
                info!("{}", format_color("Total Transactions", TRANSACTION_COUNTER.load(Ordering::Relaxed), false));
                info!("{}", format_color("Total Traces", TRACE_COUNTER.load(Ordering::Relaxed), false));
                info!("{}", format_color("Successful Parses", SUCCESSFUL_PARSE_COUNTER.load(Ordering::Relaxed), false));
                info!("{}", format_color("Empty Input Errors", self.empty_input_errors, true));
                info!("{}", format_color("ABI Parse Errors", self.abi_parse_errors, true));
                info!("{}", format_color("Invalid Function Selector Errors", self.invalid_function_selector_errors, true));
                info!("{}", format_color("ABI Decoding Failed Errors", self.abi_decoding_failed_errors, true));
                info!("{}", format_color("Trace Missing Errors", self.trace_missing_errors, true));
                self.etherscan_errors.into_info();
                println!();
            }
        );
    }
}

impl<S> Layer<S> for ParserStatsLayer
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_new_span(&self, _attrs: &Attributes<'_>, id: &Id, ctx: Context<'_, S>) {
        let span = ctx.span(id).unwrap();

        span.extensions_mut().insert(ParserErrorStats::default());
    }

    fn on_event(&self, event: &tracing::Event<'_>, ctx: tracing_subscriber::layer::Context<'_, S>) {
        if let Some(id) = ctx.current_span().id() {
            let span = ctx.span(id).unwrap();
            if let Some(ext) = span.extensions_mut().get_mut::<ParserErrorStats>() {
                //println!("bane :{:?}", event.metadata().target());
                if event.metadata().target() == "poirot::parser::stats" {
                    //ext.print_stats();
                } else {
                    event.record(&mut *ext);
                }
            };
        }
    }

    fn on_exit(&self, id: &Id, ctx: Context<'_, S>) {
        let span = ctx.span(id).unwrap();
        if span.parent().is_none() {
            //println!("bane");
            println!("span scope: {:?}", ctx.span_scope(id).map(|ss| ss.into_iter().map(|s| s.name()).collect::<Vec<&str>>()));
            if let Some(ext) = span.extensions_mut().get_mut::<ParserErrorStats>() {
                println!("bane2");
                ext.print_stats();
            }
        }
    }
}

impl Visit for ParserErrorStats {
    /// increases the counts of the numerical fields based off the event name
    fn record_debug(&mut self, _field: &Field, _value: &dyn std::fmt::Debug) {
        //self.print_stats();
    }
    
    /// incremenets the error count fields of the stats
    fn record_error(&mut self, _field: &Field, value: &(dyn std::error::Error + 'static)) {
        if let Some(error) = value.downcast_ref::<TraceParseError>() {
            match error {
                TraceParseError::TraceMissing => self.trace_missing_errors += 1,
                TraceParseError::EmptyInput(_) => self.empty_input_errors += 1,
                TraceParseError::AbiParseError(_) => self.abi_parse_errors += 1,
                TraceParseError::InvalidFunctionSelector(_) => self.abi_parse_errors += 1,
                TraceParseError::AbiDecodingFailed(_) => self.abi_decoding_failed_errors += 1,
                TraceParseError::EtherscanError(e) => {
                    self.etherscan_errors.total += 1;
                    match e {
                        EtherscanError::ChainNotSupported(_) => self.etherscan_errors.chain_not_supported += 1,
                        EtherscanError::ExecutionFailed(_) => self.etherscan_errors.execution_failed += 1,
                        EtherscanError::BalanceFailed => self.etherscan_errors.balance_failed += 1,
                        EtherscanError::NotProxy => self.etherscan_errors.not_proxy += 1,
                        EtherscanError::MissingImplementationAddress => self.etherscan_errors.missing_implementation_address += 1,
                        EtherscanError::BlockNumberByTimestampFailed => self.etherscan_errors.block_number_by_timestamp_failed += 1,
                        EtherscanError::TransactionReceiptFailed => self.etherscan_errors.transaction_receipt_failed += 1,
                        EtherscanError::GasEstimationFailed => self.etherscan_errors.gas_estimation_failed += 1,
                        EtherscanError::BadStatusCode(_) => self.etherscan_errors.bad_status_code += 1,
                        EtherscanError::EnvVarNotFound(_) => self.etherscan_errors.env_var_not_found += 1,
                        EtherscanError::Reqwest(_) => self.etherscan_errors.reqwest += 1,
                        EtherscanError::Serde(_) => self.etherscan_errors.serde += 1,
                        EtherscanError::ContractCodeNotVerified(_) => self.etherscan_errors.contract_code_not_verified += 1,
                        EtherscanError::EmptyResult { status: _, message: _ } => self.etherscan_errors.empty_result += 1,
                        EtherscanError::RateLimitExceeded => self.etherscan_errors.rate_limit_exceeded += 1,
                        EtherscanError::IO(_) => self.etherscan_errors.io += 1,
                        EtherscanError::LocalNetworksNotSupported => self.etherscan_errors.local_networks_not_supported += 1,
                        EtherscanError::ErrorResponse { status: _, message: _, result: _ } => self.etherscan_errors.error_response += 1,
                        EtherscanError::Unknown(_) => self.etherscan_errors.unknown += 1,
                        EtherscanError::Builder(_) => self.etherscan_errors.builder += 1,
                        EtherscanError::MissingSolcVersion(_) => self.etherscan_errors.missing_solc_version += 1,
                        EtherscanError::InvalidApiKey => self.etherscan_errors.invalid_api_key += 1,
                        EtherscanError::BlockedByCloudflare => self.etherscan_errors.blocked_by_cloudflare += 1,
                        EtherscanError::CloudFlareSecurityChallenge => self.etherscan_errors.cloudflare_security_challenge += 1,
                        EtherscanError::PageNotFound => self.etherscan_errors.page_not_found += 1,
                        EtherscanError::CacheError(_) => self.etherscan_errors.cache_error += 1,
                    }
                },
            }
        }
    }
}
