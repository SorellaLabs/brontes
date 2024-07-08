use std::fmt::Debug;

use ::clickhouse::{DbRow, InsertRow};
pub mod address_metadata;
pub mod address_to_protocol_info;

#[rustfmt::skip]
pub mod block_analysis;
pub mod block_times;
pub mod builder;
pub mod cex;

pub mod clickhouse;
pub mod clickhouse_serde;
pub mod codecs;
pub mod dex;
pub mod initialized_state;
pub mod metadata;
pub mod mev_block;
pub mod normalized_actions;
pub mod pool_creation_block;
pub mod redefined_types;
pub mod searcher;
pub mod token_info;
pub mod traces;
pub mod traits;

/// This table is used to add run id inserts for each clickhouse table in order
/// for us to not have to clear runs multiple times
#[derive(Debug, Clone, serde::Serialize)]
pub struct DbDataWithRunId<Table: Debug + Clone + serde::Serialize + DbRow + Sync + Send> {
    #[serde(flatten)]
    pub table:  Table,
    pub run_id: u64,
}
impl<Table: Debug + Clone + serde::Serialize + DbRow + Sync + Send> InsertRow
    for DbDataWithRunId<Table>
{
    fn get_column_names(&self) -> &'static [&'static str] {
        let inner = Table::COLUMN_NAMES;
        let mut res = Vec::new();
        for i in inner {
            res.push(*i);
        }
        res.push("run_id");
        let sliced = res.into_boxed_slice();

        Box::leak(sliced)
    }
}

#[derive(Debug, Clone, serde::Serialize, ::clickhouse::Row)]
pub struct RunId {
    pub run_id: u64,
}
