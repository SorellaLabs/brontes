use alloy_primitives::Address;
use clickhouse::Row;
use redefined::Redefined;
use rkyv::{Archive, Deserialize as rDeserialize, Serialize as rSerialize};
use serde::{Deserialize, Serialize};

use crate::{db::redefined_types::primitives::*, implement_table_value_codecs_with_zc};

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

pub struct CexBlockTimes {
    pub start_timestamp: u64,
    pub end_timestamp:   u64,
    pub block_number:    u64,
}

impl CexBlockTimes {
    pub fn trade_times(value: BlockTimes) -> Self {
        Self {
            start_timestamp: value.timestamp - 6,
            end_timestamp:   value.timestamp + 6,
            block_number:    value.block_number,
        }
    }
}
