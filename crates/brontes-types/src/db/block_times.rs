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
    pub fn trade_times(value: BlockTimes) -> Self {
        Self {
            start_timestamp: value.timestamp - 6000,
            end_timestamp:   value.timestamp + 6000,
            block_number:    value.block_number,
        }
    }

    pub fn quote_times(value: BlockTimes) -> Self {
        Self {
            start_timestamp: value.timestamp - 12000,
            end_timestamp:   value.timestamp,
            block_number:    value.block_number,
        }
    }
}
