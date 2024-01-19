use std::collections::HashMap;

use brontes_database::Pair;
use brontes_pricing::SubGraphEdge;
use redefined::RedefinedConvert;
use sorella_db_databases::clickhouse::{self, Row};

use super::redefined_types::subgraph::Redefined_SubGraphsEntry;
use crate::{LibmdbxData, SubGraphs};

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, Row)]
pub struct SubGraphsData {
    pub pair: Pair,
    pub data: SubGraphsEntry,
}

impl LibmdbxData<SubGraphs> for SubGraphsData {
    fn into_key_val(
        &self,
    ) -> (<SubGraphs as reth_db::table::Table>::Key, <SubGraphs as reth_db::table::Table>::Value)
    {
        (self.pair, Redefined_SubGraphsEntry::from_source(self.data.clone()))
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SubGraphsEntry(pub HashMap<u64, Vec<SubGraphEdge>>);
