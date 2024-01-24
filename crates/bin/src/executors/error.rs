use thiserror::Error;

#[allow(dead_code)]
#[derive(Debug, Error)]
pub enum ExecutorErrors {
    #[error("no dex pricing found for block")]
    MissingDexPrices,
}
