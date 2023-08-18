pub mod parser;
pub mod utils;

pub(crate) const UNKNOWN: &str = "unknown";
pub(crate) const RECEIVE: &str = "receive";
pub(crate) const FALLBACK: &str = "fallback";
pub(crate) const CACHE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10_000);
