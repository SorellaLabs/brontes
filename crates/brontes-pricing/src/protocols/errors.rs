use std::time::SystemTimeError;

use alloy_primitives::{Address, U256};
use alloy_sol_types::Error as AlloyError;
use thiserror::Error;
use tokio::task::JoinError;

use super::uniswap_v3::uniswap_v3_math::error::UniswapV3MathError;

#[derive(Error, Debug)]
pub enum AmmError {
    #[error(transparent)]
    CallError(#[from] eyre::Error),
    #[error("No state was found for address: {0:?}")]
    NoStateError(Address),
    #[error("Join error")]
    JoinError(#[from] JoinError),
    #[error("Serde json error")]
    SerdeJsonError(#[from] serde_json::error::Error),
    #[error("IO error")]
    IOError(#[from] std::io::Error),
    #[error("Error when converting from hex to U256")]
    FromHexError,
    #[error("Uniswap V3 math error")]
    UniswapV3MathError(#[from] UniswapV3MathError),
    #[error("Pair for token_a/token_b does not exist in provided dexes")]
    PairDoesNotExistInDexes(Address, Address),
    #[error("Could not initialize new pool from event log")]
    UnrecognizedPoolCreatedEventLog,
    #[error("Error when syncing pool")]
    SyncError(Address),
    #[error("Error when getting pool data")]
    PoolDataError,
    #[error("Arithmetic error")]
    ArithmeticError(#[from] ArithmeticError),
    #[error("No initialized ticks during v3 swap simulation")]
    NoInitializedTicks,
    #[error("No liquidity net found during v3 swap simulation")]
    NoLiquidityNet,
    #[error("Incongruent AMMS supplied to batch request")]
    IncongruentAMMs,
    #[error("Invalid ERC4626 fee")]
    InvalidERC4626Fee,
    #[error("Event log error")]
    EventLogError(#[from] EventLogError),
    #[error("Block number not found")]
    BlockNumberNotFound,
    #[error("Swap simulation error")]
    SwapSimulationError(#[from] SwapSimulationError),
    #[error("Invalid data from batch request")]
    BatchRequestError(Address),
    #[error("Checkpoint error")]
    CheckpointError(#[from] CheckpointError),
    #[error(transparent)]
    AlloyError(#[from] AlloyError),
    #[error("")]
    UnsupportedProtocol,
}

#[derive(Error, Debug)]
pub enum ArithmeticError {
    #[error("Shadow overflow")]
    ShadowOverflow(U256),
    #[error("Rounding Error")]
    RoundingError,
    #[error("Y is zero")]
    YIsZero,
    #[error("Sqrt price overflow")]
    SqrtPriceOverflow,
    #[error("U128 conversion error")]
    U128ConversionError,
    #[error("Uniswap v3 math error")]
    UniswapV3MathError(#[from] UniswapV3MathError),
    #[error("v2 div by zero")]
    UniV2DivZero,
}

#[derive(Error, Debug)]
pub enum EventLogError {
    #[error("Invalid event signature")]
    InvalidEventSignature,
    #[error("Log Block number not found")]
    LogBlockNumberNotFound,
}

#[derive(Error, Debug)]
pub enum SwapSimulationError {
    #[error("Could not get next tick")]
    InvalidTick,
    #[error("Uniswap v3 math error")]
    UniswapV3MathError(#[from] UniswapV3MathError),
    #[error("Liquidity underflow")]
    LiquidityUnderflow,
}

#[derive(Error, Debug)]
pub enum CheckpointError {
    #[error("System time error")]
    SystemTimeError(#[from] SystemTimeError),
    #[error("Serde json error")]
    SerdeJsonError(#[from] serde_json::error::Error),
    #[error("IO error")]
    IOError(#[from] std::io::Error),
}
