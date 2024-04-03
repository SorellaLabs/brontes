use clickhouse::Row;
use serde::Deserialize;

#[derive(Debug, Default, Clone, Row, PartialEq, Deserialize)]
pub struct BlockTimes {
    pub block_number: u64,
    pub timestamp:    u64,
}

impl BlockTimes {
    pub fn convert_to_timestamp_query(&self, before_block: u64, after_block: u64) -> String {
        format!(
            "(timestamp >= {} AND timestamp < {})",
            self.timestamp - before_block,
            self.timestamp + after_block
        )
    }
}

#[derive(Debug)]
pub struct CexBlockTimes {
    pub start_timestamp: u64,
    pub end_timestamp:   u64,
    pub block_number:    u64,
}

impl CexBlockTimes {
    pub fn add_time_window(value: BlockTimes, time_window: (u64, u64)) -> Self {
        Self {
            start_timestamp: value.timestamp - time_window.0 * 1000000,
            end_timestamp:   value.timestamp + time_window.1 * 1000000,
            block_number:    value.block_number,
        }
    }
}
