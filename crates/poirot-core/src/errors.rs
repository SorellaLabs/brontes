use alloy_etherscan::errors::EtherscanError;
use ethers_core::types::H256;
use thiserror::Error;

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

/// these are needed so we can implement the async tracing
/// they are also Send + Sync safe as they are just nested Enums with Send + Sync safe types
unsafe impl Send for TraceParseError {}
unsafe impl Sync for TraceParseError {}
