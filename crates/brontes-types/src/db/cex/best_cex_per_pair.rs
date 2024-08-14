use clickhouse::Row;
use serde::{Deserialize, Serialize};

use super::CexExchange;
use crate::serde_utils::cex_exchange_vec;

/// stores the best cex (most volume on the pair).
/// this is used to choose what cex is most likely the
/// driver of true price
#[derive(Debug, Clone, Row, Serialize, Deserialize)]
pub struct BestCexPerPair {
    pub symbol:    String,
    #[serde(with = "cex_exchange_vec")]
    pub exchange:  Vec<CexExchange>,
    pub timestamp: u64,
}
