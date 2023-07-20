use alloy_etherscan::errors::EtherscanError;
use colored::*;
use reth_primitives::{BlockNumberOrTag, H256, U256};

/// Custom error type for trace parsing
#[derive(Debug)]
pub enum TraceParseError {
    TraceMissing,
    NotRecognizedAction(H256), 
    EmptyInput(H256),          
    EtherscanError(EtherscanError),
    AbiParseError(serde_json::Error),
    InvalidFunctionSelector(H256),
    AbiDecodingFailed(H256),
}

#[derive(Default, Debug)]
pub struct ParserStats {
    pub block_number: BlockNumberOrTag,
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
