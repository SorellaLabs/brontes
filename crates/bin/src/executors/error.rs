use thiserror::Error;

#[derive(Debug, Error)]
pub enum ExecutorErrors {
    #[error("no dex pricing found for block")]
    MissingDexPrices,
}
