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
    #[error("send error to prometheus")]
    ChannelSendError(String),
}

impl From<EtherscanError> for TraceParseError {
    fn from(err: EtherscanError) -> TraceParseError {
        TraceParseError::EtherscanError(err)
    }
}



/// enum for error
#[derive(Debug, Clone, Copy)]
pub enum TraceParseErrorKind {
    TraceMissing,
    EmptyInput,
    AbiParseError,
    InvalidFunctionSelector,
    AbiDecodingFailed,
    ChannelSendError,
    EtherscanChainNotSupported,
    EtherscanExecutionFailed,
    EtherscanBalanceFailed,
    EtherscanNotProxy,
    EtherscanMissingImplementationAddress,
    EtherscanBlockNumberByTimestampFailed,
    EtherscanTransactionReceiptFailed,
    EtherscanGasEstimationFailed,
    EtherscanBadStatusCode,
    EtherscanEnvVarNotFound,
    EtherscanReqwest,
    EtherscanSerde,
    EtherscanContractCodeNotVerified,
    EtherscanEmptyResult,
    EtherscanRateLimitExceeded,
    EtherscanIO,
    EtherscanLocalNetworksNotSupported,
    EtherscanErrorResponse,
    EtherscanUnknown,
    EtherscanBuilder,
    EtherscanMissingSolcVersion,
    EtherscanInvalidApiKey,
    EtherscanBlockedByCloudflare,
    EtherscanCloudFlareSecurityChallenge,
    EtherscanPageNotFound,
    EtherscanCacheError,
}



impl From<TraceParseError> for TraceParseErrorKind {
    fn from(err: TraceParseError) -> TraceParseErrorKind {
        match err {
            TraceParseError::TraceMissing => TraceParseErrorKind::TraceMissing,
            TraceParseError::EmptyInput(_) => TraceParseErrorKind::EmptyInput,
            TraceParseError::EtherscanError(e) => {
                match e {
                    EtherscanError::ChainNotSupported(_) => TraceParseErrorKind::EtherscanChainNotSupported,
                    EtherscanError::ExecutionFailed(_) => TraceParseErrorKind::EtherscanExecutionFailed,
                    EtherscanError::BalanceFailed => TraceParseErrorKind::EtherscanBalanceFailed,
                    EtherscanError::NotProxy => TraceParseErrorKind::EtherscanNotProxy,
                    EtherscanError::MissingImplementationAddress => TraceParseErrorKind::EtherscanMissingImplementationAddress,
                    EtherscanError::BlockNumberByTimestampFailed => TraceParseErrorKind::EtherscanBlockNumberByTimestampFailed,
                    EtherscanError::TransactionReceiptFailed => TraceParseErrorKind::EtherscanTransactionReceiptFailed,
                    EtherscanError::GasEstimationFailed => TraceParseErrorKind::EtherscanGasEstimationFailed,
                    EtherscanError::BadStatusCode(_) => TraceParseErrorKind::EtherscanBadStatusCode,
                    EtherscanError::EnvVarNotFound(_) => TraceParseErrorKind::EtherscanEnvVarNotFound,
                    EtherscanError::Reqwest(_) => TraceParseErrorKind::EtherscanReqwest,
                    EtherscanError::Serde(_) => TraceParseErrorKind::EtherscanSerde,
                    EtherscanError::ContractCodeNotVerified(_) => TraceParseErrorKind::EtherscanContractCodeNotVerified,
                    EtherscanError::EmptyResult { .. } => TraceParseErrorKind::EtherscanEmptyResult,
                    EtherscanError::RateLimitExceeded => TraceParseErrorKind::EtherscanRateLimitExceeded,
                    EtherscanError::IO(_) => TraceParseErrorKind::EtherscanIO,
                    EtherscanError::LocalNetworksNotSupported => TraceParseErrorKind::EtherscanLocalNetworksNotSupported,
                    EtherscanError::ErrorResponse { .. } => TraceParseErrorKind::EtherscanErrorResponse,
                    EtherscanError::Unknown(_) => TraceParseErrorKind::EtherscanUnknown,
                    EtherscanError::Builder(_) => TraceParseErrorKind::EtherscanBuilder,
                    EtherscanError::MissingSolcVersion(_) => TraceParseErrorKind::EtherscanMissingSolcVersion,
                    EtherscanError::InvalidApiKey => TraceParseErrorKind::EtherscanInvalidApiKey,
                    EtherscanError::BlockedByCloudflare => TraceParseErrorKind::EtherscanBlockedByCloudflare,
                    EtherscanError::CloudFlareSecurityChallenge => TraceParseErrorKind::EtherscanCloudFlareSecurityChallenge,
                    EtherscanError::PageNotFound => TraceParseErrorKind::EtherscanPageNotFound,
                    EtherscanError::CacheError(_) => TraceParseErrorKind::EtherscanCacheError,
                }
            }
            TraceParseError::AbiParseError(_) => TraceParseErrorKind::AbiParseError,
            TraceParseError::InvalidFunctionSelector(_) => TraceParseErrorKind::InvalidFunctionSelector,
            TraceParseError::AbiDecodingFailed(_) => TraceParseErrorKind::AbiDecodingFailed,
            TraceParseError::ChannelSendError(_) => TraceParseErrorKind::ChannelSendError,
        }
    }
}
