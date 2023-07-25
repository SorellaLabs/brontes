use ethers_core::types::H256;
use thiserror::Error;
use alloy_etherscan::errors::EtherscanError;
use tracing::Value;



/// Custom error type
#[derive(Debug, Error)]
pub enum TraceParseError {
    #[error("trace missing")]
    TraceMissing,
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

/*

/// Custom enum error for tracing
#[derive(Debug, Error)]
pub enum TraceParseError {
    #[error("trace missing")]
    TraceMissing,
    #[error("empty input")]
    EmptyInput,
    #[error("etherscan error")]
    EtherscanError,
    #[error("abi parse error")]
    AbiParseError,
    #[error("invalid function selector")]
    InvalidFunctionSelector,
    #[error("abi decoding failed")]
    AbiDecodingFailed,
}


impl From<ParseError> for TraceParseError {
    fn from(err: ParseError) -> TraceParseError {
        match err {
            ParseError::TraceMissing => TraceParseError::TraceMissing,
            ParseError::EmptyInput(_) => TraceParseError::EmptyInput,
            ParseError::EtherscanError(_) => TraceParseError::EtherscanError,
            ParseError::AbiParseError(_) => TraceParseError::AbiParseError,
            ParseError::InvalidFunctionSelector(_) => TraceParseError::InvalidFunctionSelector,
            ParseError::AbiDecodingFailed(_) => TraceParseError::AbiDecodingFailed,
        }
    }
}

impl Value for TraceParseError {
    fn record(&self, key: &tracing::field::Field, visitor: &mut dyn tracing::field::Visit) {
        visitor.record_error(key, self)
    }
}

 */