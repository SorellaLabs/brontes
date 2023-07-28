use std::{sync::{atomic::Ordering, Mutex}, collections::HashMap};

use crate::{format_color, errors::TraceParseError, *};
use alloy_etherscan::errors::EtherscanError;
use colored::Color;
use revm_primitives::B256;
use serde_json::{Value, json};
use serde::Serialize;
use tracing::{
    field::{Field, Visit},
    span::Attributes,
    Id, Subscriber, info, Level,
};
use tracing_subscriber::{layer::Context, registry::LookupSpan, Layer, EnvFilter, FmtSubscriber};
use lazy_static::*;
pub struct ParserStatsLayer;


lazy_static! {
    pub static ref BLOCK_STATS: Mutex<HashMap<u64, BlockStats>> = {
        Mutex::new(HashMap::new())
    };

    pub static ref TX_STATS: Mutex<HashMap<B256, TransactionStats>> = {
        Mutex::new(HashMap::new())
    };
}

pub struct BlockStats {
    pub block_num: u64,
    pub tx_stats: Vec<TransactionStats>,
}

impl BlockStats {
    pub fn display_stats(&self) {

        println!("{}", format_color("STATS FOR BLOCK", self.block_num as usize, Color::BrightBlue).bold());
        println!("----------------------------------------------------------------------------------------");
        println!("{}", format_color("Total Transactions", self.tx_stats.len(), Color::Blue));
        println!("{}", format_color("Total Traces", self.tx_stats.iter().map(|tx| tx.error_parses.len() + tx.successful_parses).sum::<usize>(), Color::Blue));
        println!("{}", format_color("Successful Parses", self.tx_stats.iter().map(|tx| tx.successful_parses).sum::<usize>(), Color::Blue));
        println!("{}", format_color("Total Errors", self.tx_stats.iter().map(|tx| tx.error_parses.len()).sum::<usize>(), Color::Blue));

        let mut errors = ErrorStats::default();
        for err in self.tx_stats.iter().map(|tx| &tx.error_parses).flatten() {
            errors.count_error(err.error.as_ref())
        }
        errors.display_stats(Color::Blue, "");
        println!();
    }
}

pub struct TransactionStats {
    pub tx_hash: B256,
    pub successful_parses: usize,
    pub error_parses: Vec<TraceStat>,
}

impl TransactionStats {
    pub fn display_stats(&self) {
        let spacing = " ".repeat(8);

        println!("{}{}", spacing, format_color("STATS FOR TRANSACTION", format!("{:#x}", self.tx_hash), Color::BrightCyan).bold());
        println!("{}----------------------------------------------------------------------------------------", spacing);
        println!("{}{}", spacing, format_color("Total Traces", self.successful_parses + self.error_parses.len(), Color::Cyan));
        println!("{}{}", spacing, format_color("Successful Parses", self.successful_parses, Color::Cyan));
        println!("{}{}", spacing, format_color("Total Errors", self.error_parses.len(), Color::Cyan));

        let mut errors = ErrorStats::default();
        for err in &self.error_parses {
            errors.count_error(err.error.as_ref())
        }
        errors.display_stats(Color::Cyan, &spacing);

        for trace in &self.error_parses {
            println!("{}{}", spacing.repeat(1), format!("{} - {:?}", format_color("Error - Trace", trace.idx, Color::Cyan), trace.error));
        }
        println!();
    }
}

pub struct TraceStat {
    pub idx: usize,
    pub error: Box<dyn std::error::Error + Sync + Send +'static>
}


#[derive(Debug, Default, Serialize)]
pub struct ErrorStats {
    pub empty_input_errors: usize,
    pub abi_parse_errors: usize,
    pub invalid_function_selector_errors: usize,
    pub abi_decoding_failed_errors: usize,
    pub trace_missing_errors: usize,
    pub etherscan_chain_not_supported: usize,
    pub etherscan_execution_failed: usize,
    pub etherscan_balance_failed: usize,
    pub etherscan_not_proxy: usize,
    pub etherscan_missing_implementation_address: usize,
    pub etherscan_block_number_by_timestamp_failed: usize,
    pub etherscan_transaction_receipt_failed: usize,
    pub etherscan_gas_estimation_failed: usize,
    pub etherscan_bad_status_code: usize,
    pub etherscan_env_var_not_found: usize,
    pub etherscan_reqwest: usize,
    pub etherscan_serde: usize,
    pub etherscan_contract_code_not_verified: usize,
    pub etherscan_empty_result: usize,
    pub etherscan_rate_limit_exceeded: usize,
    pub etherscan_io: usize,
    pub etherscan_local_networks_not_supported: usize,
    pub etherscan_error_response: usize,
    pub etherscan_unknown: usize,
    pub etherscan_builder: usize,
    pub etherscan_missing_solc_version: usize,
    pub etherscan_invalid_api_key: usize,
    pub etherscan_blocked_by_cloudflare: usize,
    pub etherscan_cloudflare_security_challenge: usize,
    pub etherscan_page_not_found: usize,
    pub etherscan_cache_error: usize,
}

impl ErrorStats {
    fn count_error(&mut self, error: &(dyn std::error::Error + Send + Sync + 'static)) {
        if let Some(error) = error.downcast_ref::<TraceParseError>() {
            match error {
                TraceParseError::TraceMissing => self.trace_missing_errors += 1,
                TraceParseError::EmptyInput(_) => self.empty_input_errors += 1,
                TraceParseError::AbiParseError(_) => self.abi_parse_errors += 1,
                TraceParseError::InvalidFunctionSelector(_) => self.abi_parse_errors += 1,
                TraceParseError::AbiDecodingFailed(_) => self.abi_decoding_failed_errors += 1,
                TraceParseError::EtherscanError(e) => {
                    match e {
                        EtherscanError::ChainNotSupported(_) => self.etherscan_chain_not_supported += 1,
                        EtherscanError::ExecutionFailed(_) => self.etherscan_execution_failed += 1,
                        EtherscanError::BalanceFailed => self.etherscan_balance_failed += 1,
                        EtherscanError::NotProxy => self.etherscan_not_proxy += 1,
                        EtherscanError::MissingImplementationAddress => self.etherscan_missing_implementation_address += 1,
                        EtherscanError::BlockNumberByTimestampFailed => self.etherscan_block_number_by_timestamp_failed += 1,
                        EtherscanError::TransactionReceiptFailed => self.etherscan_transaction_receipt_failed += 1,
                        EtherscanError::GasEstimationFailed => self.etherscan_gas_estimation_failed += 1,
                        EtherscanError::BadStatusCode(_) => self.etherscan_bad_status_code += 1,
                        EtherscanError::EnvVarNotFound(_) => self.etherscan_env_var_not_found += 1,
                        EtherscanError::Reqwest(_) => self.etherscan_reqwest += 1,
                        EtherscanError::Serde(_) => self.etherscan_serde += 1,
                        EtherscanError::ContractCodeNotVerified(_) => self.etherscan_contract_code_not_verified += 1,
                        EtherscanError::EmptyResult { status: _, message: _ } => self.etherscan_empty_result += 1,
                        EtherscanError::RateLimitExceeded => self.etherscan_rate_limit_exceeded += 1,
                        EtherscanError::IO(_) => self.etherscan_io += 1,
                        EtherscanError::LocalNetworksNotSupported => self.etherscan_local_networks_not_supported += 1,
                        EtherscanError::ErrorResponse { status: _, message: _, result: _ } => self.etherscan_error_response += 1,
                        EtherscanError::Unknown(_) => self.etherscan_unknown += 1,
                        EtherscanError::Builder(_) => self.etherscan_builder += 1,
                        EtherscanError::MissingSolcVersion(_) => self.etherscan_missing_solc_version += 1,
                        EtherscanError::InvalidApiKey => self.etherscan_invalid_api_key += 1,
                        EtherscanError::BlockedByCloudflare => self.etherscan_blocked_by_cloudflare += 1,
                        EtherscanError::CloudFlareSecurityChallenge => self.etherscan_cloudflare_security_challenge += 1,
                        EtherscanError::PageNotFound => self.etherscan_page_not_found += 1,
                        EtherscanError::CacheError(_) => self.etherscan_cache_error += 1,
                    }
                }
            }
        }
    }


    fn display_stats(&self, color: Color, spacing: &str) {
        let json_value: Value = json!(self);
        if let Value::Object(map) = json_value {
            for (key, value) in map {
                if let Value::Number(num) = value {
                    if num.as_u64().unwrap_or(0) > 0 {
                        println!("{}{}", spacing, format_color(&key, num.as_u64().unwrap() as usize, color));
                    }
                }
            }
        }
    }
}



pub fn display_all_stats() {
    let stats = BLOCK_STATS.lock().unwrap();

    display_total_stats(stats.iter().map(|s| s.1).collect());

    for (_, block_stat) in stats.iter() {
        block_stat.display_stats();
        
        for tx_stat in &block_stat.tx_stats {
            tx_stat.display_stats();
        }
    }
}


pub fn display_total_stats(blocks: Vec<&BlockStats>) {
    println!("{}", format!("ALL STATS").bright_yellow().bold());
    println!("----------------------------------------------------------------------------------------");
    println!("----------------------------------------------------------------------------------------");
    println!("{}", format_color("Total Blocks", blocks.len(), Color::Yellow));
    println!("{}", format_color("Total Transactions", blocks.iter().map(|b| b.tx_stats.len()).sum::<usize>(), Color::Yellow));
    println!("{}", format_color("Total Traces", blocks.iter().map(|b| b.tx_stats.iter().map(|tx| tx.error_parses.len() + tx.successful_parses).sum::<usize>()).sum::<usize>(), Color::Yellow));
    println!("{}", format_color("Successful Parses", blocks.iter().map(|b| b.tx_stats.iter().map(|tx| tx.successful_parses).sum::<usize>()).sum::<usize>(), Color::Yellow));
    println!("{}", format_color("Total Errors", blocks.iter().map(|b| b.tx_stats.iter().map(|tx| tx.error_parses.len()).sum::<usize>()).sum::<usize>(), Color::Yellow));

    let mut errors = ErrorStats::default();
    for err in blocks.iter().map(|b| b.tx_stats.iter().map(|tx| &tx.error_parses).flatten()).flatten() {
        errors.count_error(err.error.as_ref())
    }
    errors.display_stats(Color::Yellow, "");
    println!();
}