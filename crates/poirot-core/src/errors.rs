use std::sync::Arc;

use alloy_etherscan::errors::EtherscanError;
use ethers_core::types::H256;
use poirot_metrics::trace::types::TraceParseErrorKind;
use reth_rpc::eth::error::EthApiError;
use thiserror::Error;

/// Custom error type
#[derive(Debug, Error)]
pub enum TraceParseError {
    #[error("trace missing in block {0}")]
    TracesMissingBlock(u64),
    #[error("trace missing in transaction {0}")]
    TracesMissingTx(H256),
    #[error("empty input: {0}")]
    EmptyInput(H256),
    #[error("etherscan error: {0}")]
    EtherscanError(Arc<EtherscanError>),
    #[error("abi parse error: {0}")]
    AbiParseError(serde_json::Error),
    #[error("invalid function selector: {0}")]
    InvalidFunctionSelector(H256),
    #[error("abi decoding failed: {0}")]
    AbiDecodingFailed(H256),
    #[error("send error to prometheus")]
    ChannelSendError(String),
    #[error("trace missing")]
    EthApiError(EthApiError)
}

impl From<EtherscanError> for TraceParseError {
    fn from(err: EtherscanError) -> TraceParseError {
        TraceParseError::EtherscanError(Arc::new(err))
    }
}

impl From<EthApiError> for TraceParseError {
    fn from(err: EthApiError) -> TraceParseError {
        TraceParseError::EthApiError(err)
    }
}

/// TODO: why don't we just use the default error here since we are litterally
/// just mapping 1-1 and dropping some state.
impl From<&TraceParseError> for TraceParseErrorKind {
    fn from(val: &TraceParseError) -> Self {
        match val {
            TraceParseError::TracesMissingBlock(_) => TraceParseErrorKind::TracesMissingBlock,
            TraceParseError::TracesMissingTx(_) => TraceParseErrorKind::TracesMissingTx,
            TraceParseError::EmptyInput(_) => TraceParseErrorKind::EmptyInput,
            TraceParseError::EthApiError(e) => match e {
                EthApiError::EmptyRawTransactionData => {
                    TraceParseErrorKind::EthApiEmptyRawTransactionData
                }
                EthApiError::FailedToDecodeSignedTransaction => {
                    TraceParseErrorKind::EthApiFailedToDecodeSignedTransaction
                }
                EthApiError::InvalidTransactionSignature => {
                    TraceParseErrorKind::EthApiInvalidTransactionSignature
                }
                EthApiError::UnknownSafeOrFinalizedBlock => {
                    TraceParseErrorKind::EthApiUnknownSafeOrFinalizedBlock
                }
                EthApiError::ExecutionTimedOut(_) => TraceParseErrorKind::EthApiExecutionTimedOut,
                EthApiError::CallInputError(_) => TraceParseErrorKind::EthApiCallInputError,

                EthApiError::PoolError(_) => TraceParseErrorKind::EthApiPoolError,
                EthApiError::UnknownBlockNumber => TraceParseErrorKind::EthApiUnknownBlockNumber,
                EthApiError::UnknownBlockOrTxIndex => {
                    TraceParseErrorKind::EthApiUnknownBlockOrTxIndex
                }
                EthApiError::InvalidBlockRange => TraceParseErrorKind::EthApiInvalidBlockRange,
                EthApiError::PrevrandaoNotSet => TraceParseErrorKind::EthApiPrevrandaoNotSet,
                EthApiError::ConflictingFeeFieldsInRequest => {
                    TraceParseErrorKind::EthApiConflictingFeeFieldsInRequest
                }
                EthApiError::InvalidTransaction(_) => TraceParseErrorKind::EthApiInvalidTransaction,
                EthApiError::InvalidBlockData(_) => TraceParseErrorKind::EthApiInvalidBlockData,
                EthApiError::BothStateAndStateDiffInOverride(_) => {
                    TraceParseErrorKind::EthApiBothStateAndStateDiffInOverride
                }
                EthApiError::Internal(_) => TraceParseErrorKind::EthApiInternal,
                EthApiError::Signing(_) => TraceParseErrorKind::EthApiSigning,
                EthApiError::TransactionNotFound => TraceParseErrorKind::EthApiTransactionNotFound,
                EthApiError::Unsupported(_) => TraceParseErrorKind::EthApiUnsupported,
                EthApiError::InvalidParams(_) => TraceParseErrorKind::EthApiInvalidParams,
                EthApiError::InvalidTracerConfig => TraceParseErrorKind::EthApiInvalidTracerConfig,
                EthApiError::InvalidRewardPercentiles => {
                    TraceParseErrorKind::EthApiInvalidRewardPercentiles
                }
                EthApiError::InternalTracingError => {
                    TraceParseErrorKind::EthApiInternalTracingError
                }
                EthApiError::InternalEthError => TraceParseErrorKind::EthApiInternalEthError,
                EthApiError::InternalJsTracerError(_) => {
                    TraceParseErrorKind::EthApiInternalJsTracerError
                }
            },
            TraceParseError::EtherscanError(e) => match e.as_ref() {
                EtherscanError::ChainNotSupported(_) => {
                    TraceParseErrorKind::EtherscanChainNotSupported
                }
                EtherscanError::ExecutionFailed(_) => TraceParseErrorKind::EtherscanExecutionFailed,
                EtherscanError::BalanceFailed => TraceParseErrorKind::EtherscanBalanceFailed,
                EtherscanError::NotProxy => TraceParseErrorKind::EtherscanNotProxy,
                EtherscanError::MissingImplementationAddress => {
                    TraceParseErrorKind::EtherscanMissingImplementationAddress
                }
                EtherscanError::BlockNumberByTimestampFailed => {
                    TraceParseErrorKind::EtherscanBlockNumberByTimestampFailed
                }
                EtherscanError::TransactionReceiptFailed => {
                    TraceParseErrorKind::EtherscanTransactionReceiptFailed
                }
                EtherscanError::GasEstimationFailed => {
                    TraceParseErrorKind::EtherscanGasEstimationFailed
                }
                EtherscanError::BadStatusCode(_) => TraceParseErrorKind::EtherscanBadStatusCode,
                EtherscanError::EnvVarNotFound(_) => TraceParseErrorKind::EtherscanEnvVarNotFound,
                EtherscanError::Reqwest(_) => TraceParseErrorKind::EtherscanReqwest,
                EtherscanError::Serde(_) => TraceParseErrorKind::EtherscanSerde,
                EtherscanError::ContractCodeNotVerified(_) => {
                    TraceParseErrorKind::EtherscanContractCodeNotVerified
                }
                EtherscanError::EmptyResult { .. } => TraceParseErrorKind::EtherscanEmptyResult,
                EtherscanError::RateLimitExceeded => {
                    TraceParseErrorKind::EtherscanRateLimitExceeded
                }
                EtherscanError::IO(_) => TraceParseErrorKind::EtherscanIO,
                EtherscanError::LocalNetworksNotSupported => {
                    TraceParseErrorKind::EtherscanLocalNetworksNotSupported
                }
                EtherscanError::ErrorResponse { .. } => TraceParseErrorKind::EtherscanErrorResponse,
                EtherscanError::Unknown(_) => TraceParseErrorKind::EtherscanUnknown,
                EtherscanError::Builder(_) => TraceParseErrorKind::EtherscanBuilder,
                EtherscanError::MissingSolcVersion(_) => {
                    TraceParseErrorKind::EtherscanMissingSolcVersion
                }
                EtherscanError::InvalidApiKey => TraceParseErrorKind::EtherscanInvalidApiKey,
                EtherscanError::BlockedByCloudflare => {
                    TraceParseErrorKind::EtherscanBlockedByCloudflare
                }
                EtherscanError::CloudFlareSecurityChallenge => {
                    TraceParseErrorKind::EtherscanCloudFlareSecurityChallenge
                }
                EtherscanError::PageNotFound => TraceParseErrorKind::EtherscanPageNotFound,
                EtherscanError::CacheError(_) => TraceParseErrorKind::EtherscanCacheError
            },
            TraceParseError::AbiParseError(_) => TraceParseErrorKind::AbiParseError,
            TraceParseError::InvalidFunctionSelector(_) => {
                TraceParseErrorKind::InvalidFunctionSelector
            }
            TraceParseError::AbiDecodingFailed(_) => TraceParseErrorKind::AbiDecodingFailed,
            TraceParseError::ChannelSendError(_) => TraceParseErrorKind::ChannelSendError
        }
    }
}
