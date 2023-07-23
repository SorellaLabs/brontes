use alloy_etherscan::errors::EtherscanError;
use colored::*;
use reth_primitives::{BlockNumberOrTag, H256, U256};
use std::fmt::Display;
use std::{collections::HashMap, sync::{Arc, Mutex}};
use tracing::field::{Field, Visit};
use tracing_subscriber::layer::{Context, Layer};
use tracing::{Event, Metadata};
use tracing_subscriber::Registry;
use alloy_json_abi::AbiItem::Error;
use thiserror::Error;


/// Enum to represent the type of TraceParseError without the associated data.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TraceParseErrorKind {
    TraceMissing,
    NotRecognizedAction,
    EmptyInput,
    EtherscanError,
    AbiParseError,
    InvalidFunctionSelector,
    AbiDecodingFailed,
}

/// Custom error type for trace parsing
#[derive(Debug, Error)]
pub enum TraceParseError {
    #[error("trace missing")]
    TraceMissing,
    #[error("not recognized action: {0}")]
    NotRecognizedAction(H256),
    #[error("empty input: {0}")]
    EmptyInput(H256),
    #[error("etherscan error: {0}")]
    EtherscanError(EtherscanError),
    #[error("abi parse error: {0}")]
    AbiParseError(serde_json::Error),
    #[error("invalid function selector: {0}")]
    InvalidFunctionSelector(H256),
    #[error("abi decoding failed: {0}")]
    AbiDecodingFailed(H256),
}

impl From<EtherscanError> for TraceParseError {
    fn from(err: EtherscanError) -> TraceParseError {
        TraceParseError::EtherscanError(err)
    }
    
}

#[derive(Default, Debug)]
pub struct ParserStats {
    pub total_tx: usize,
    pub total_traces: usize,
    pub successful_parses: usize,
    pub not_call_action_errors: usize,
    pub empty_input_errors: usize,
    pub etherscan_errors: usize,
    pub abi_parse_errors: usize,
    pub invalid_function_selector_errors: usize,
    pub abi_decoding_failed_errors: usize,
    pub trace_missing_errors: usize,
}

impl ParserStats {
    pub fn increment_error(&mut self, error: TraceParseError) {
        match error {
            TraceParseError::NotRecognizedAction(_) => self.not_call_action_errors += 1,
            TraceParseError::EmptyInput(_) => self.empty_input_errors += 1,
            TraceParseError::EtherscanError(_) => self.etherscan_errors += 1,
            TraceParseError::AbiParseError(_) => self.abi_parse_errors += 1,
            TraceParseError::InvalidFunctionSelector(_) => {
                self.invalid_function_selector_errors += 1
            }
            TraceParseError::AbiDecodingFailed(_) => self.abi_parse_errors += 1,
            TraceParseError::TraceMissing => self.trace_missing_errors += 1,
        };
    }

    pub fn merge(&mut self, other: &ParserStats) {
        self.total_tx += other.total_tx;
        self.total_traces += other.total_traces;
        self.successful_parses += other.successful_parses;
        self.not_call_action_errors += other.not_call_action_errors;
        self.empty_input_errors += other.empty_input_errors;
        self.etherscan_errors += other.etherscan_errors;
        self.abi_parse_errors += other.abi_parse_errors;
        self.invalid_function_selector_errors += other.invalid_function_selector_errors;
        self.abi_decoding_failed_errors += other.abi_decoding_failed_errors;
        self.trace_missing_errors += other.trace_missing_errors;
    }

    pub fn increment_success(&mut self) {
        self.successful_parses += 1;
    }

    pub fn display(&self) {
        println!("{}", "Parser Statistics".bold().underline());
        println!("{}: {}", "Total Traces".green().bold(), self.total_traces.to_string().cyan());
        println!(
            "{}: {}",
            "Successful Parses".green().bold(),
            self.successful_parses.to_string().cyan()
        );
        println!(
            "{}: {}",
            "Not Call Action Errors".red().bold(),
            self.not_call_action_errors.to_string().cyan()
        );
        println!(
            "{}: {}",
            "Empty Input Errors".red().bold(),
            self.empty_input_errors.to_string().cyan()
        );
        println!(
            "{}: {}",
            "Etherscan Errors".red().bold(),
            self.etherscan_errors.to_string().cyan()
        );
        println!(
            "{}: {}",
            "ABI Parse Errors".red().bold(),
            self.abi_parse_errors.to_string().cyan()
        );
        println!(
            "{}: {}",
            "Invalid Function Selector Errors".red().bold(),
            self.invalid_function_selector_errors.to_string().cyan()
        );
        println!(
            "{}: {}",
            "ABI Decoding Failed Errors".red().bold(),
            self.abi_decoding_failed_errors.to_string().cyan()
        );
    }
}



pub struct ParserStatsLayer {
    // Change this to a HashMap
    stats: Arc<Mutex<HashMap<BlockNumberOrTag, ParserStats>>>,
}



impl<S> Layer<S> for ParserStatsLayer
where
    S: tracing::Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
{
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let mut visitor = TraceParseErrorVisitor {
            kind: None,
        };
        event.record(&mut visitor);

        if let Some(kind) = visitor.kind {
            let mut stats = self.stats.lock().unwrap();
            match kind {
                TraceParseErrorKind::NotRecognizedAction => stats.not_call_action_errors += 1,
                TraceParseErrorKind::EmptyInput => stats.empty_input_errors += 1,
                TraceParseErrorKind::EtherscanError => stats.etherscan_errors += 1,
                TraceParseErrorKind::AbiParseError => stats.abi_parse_errors += 1,
                TraceParseErrorKind::InvalidFunctionSelector => stats.invalid_function_selector_errors += 1,
                TraceParseErrorKind::AbiDecodingFailed => stats.abi_decoding_failed_errors += 1,
                TraceParseErrorKind::TraceMissing => stats.trace_missing_errors += 1,
            };
        }
    }
}

struct TraceParseErrorVisitor {
    kind: Option<TraceParseErrorKind>,
}

impl Visit for TraceParseErrorVisitor {
    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "error" {
            self.kind = match value {
                "trace_missing_error" => Some(TraceParseErrorKind::TraceMissing),
                "not_call_action_error" => Some(TraceParseErrorKind::NotRecognizedAction),
                "empty_input_error" => Some(TraceParseErrorKind::EmptyInput),
                "etherscan_error" => Some(TraceParseErrorKind::EtherscanError),
                "abi_parse_error" => Some(TraceParseErrorKind::AbiParseError),
                "invalid_function_selector_error" => Some(TraceParseErrorKind::InvalidFunctionSelector),
                "abi_decoding_failed_error" => Some(TraceParseErrorKind::AbiDecodingFailed),
                _ => None,
            };
        }
    }


    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        unreachable!("This should never be called")
    }
}
