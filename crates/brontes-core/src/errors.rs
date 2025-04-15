use alloy_primitives::B256;
use brontes_metrics::trace::types::TraceParseErrorKind;
use reth_rpc::eth::error::EthApiError;
use thiserror::Error;

/// Custom error type
#[derive(Debug, Error)]
pub enum TraceParseError {
    #[error("trace missing in block {0}")]
    TracesMissingBlock(u64),
    #[error("trace missing in transaction {0}")]
    TracesMissingTx(B256),
    #[error("empty input: {0}")]
    EmptyInput(B256),
    #[error("abi parse error: {0}")]
    AbiParseError(serde_json::Error),
    #[error("invalid function selector: {0}")]
    InvalidFunctionSelector(B256),
    #[error("abi decoding failed: {0}")]
    AbiDecodingFailed(B256),
    #[error("send error to prometheus")]
    ChannelSendError(String),
    #[error("trace missing")]
    EthApiError(EthApiError),
    #[error("alloy error {0}")]
    AlloyError(alloy_dyn_abi::Error),
    #[error(transparent)]
    Eyre(#[from] eyre::Report),
}

impl From<EthApiError> for TraceParseError {
    fn from(err: EthApiError) -> TraceParseError {
        TraceParseError::EthApiError(err)
    }
}

impl From<alloy_dyn_abi::Error> for TraceParseError {
    fn from(err: alloy_dyn_abi::Error) -> TraceParseError {
        //TraceParseError::EthApiError(err)
        TraceParseError::AlloyError(err)
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
                EthApiError::InternalEthError => TraceParseErrorKind::EthApiInternalEthError,
                EthApiError::InternalJsTracerError(_) => {
                    TraceParseErrorKind::EthApiInternalJsTracerError
                }
                _ => TraceParseErrorKind::EthApiInternalJsTracerError,
            },
            TraceParseError::AbiParseError(_) => TraceParseErrorKind::AbiParseError,
            TraceParseError::InvalidFunctionSelector(_) => {
                TraceParseErrorKind::InvalidFunctionSelector
            }
            TraceParseError::AbiDecodingFailed(_) => TraceParseErrorKind::AbiDecodingFailed,
            TraceParseError::ChannelSendError(_) => TraceParseErrorKind::ChannelSendError,
            TraceParseError::AlloyError(_) => TraceParseErrorKind::AlloyError,
            TraceParseError::Eyre(_) => TraceParseErrorKind::Eyre,
        }
    }
}
