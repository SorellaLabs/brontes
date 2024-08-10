use clickhouse::Row;
use serde::Deserialize;

#[derive(Debug, Default, Clone, Row, PartialEq, Deserialize)]
pub struct BlockTimes {
    pub block_number: u64,
    pub timestamp:    u64,
}

impl BlockTimes {
    pub fn convert_to_timestamp_query(&self, before_block: f64, after_block: f64) -> String {
        format!(
            "(c.timestamp >= {} AND c.timestamp < {})",
            self.timestamp as f64 - before_block,
            self.timestamp as f64 + after_block
        )
    }
}

#[derive(Debug, Hash, PartialEq, Eq)]
pub struct CexBlockTimes {
    pub start_timestamp:   u64,
    pub end_timestamp:     u64,
    pub precise_timestamp: u64,
    pub block_number:      u64,
}

impl CexBlockTimes {
    pub fn add_time_window(value: BlockTimes, time_window: (f64, f64)) -> Self {
        Self {
            start_timestamp:   (value.timestamp as f64 - time_window.0 * 1000000.0) as u64,
            end_timestamp:     (value.timestamp as f64 + time_window.1 * 1000000.0) as u64,
            precise_timestamp: value.timestamp,
            block_number:      value.block_number,
        }
    }

    /// Returns true if supplied timestamp is within the blocks time window
    pub fn contains_time(&self, timestamp: u64) -> bool {
        timestamp >= self.start_timestamp && timestamp <= self.end_timestamp
    }

    // pub
}
