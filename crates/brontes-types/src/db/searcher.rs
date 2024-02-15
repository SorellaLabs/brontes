use redefined::{self_convert_redefined, Redefined};
use rkyv::{Archive, Deserialize as rDeserialize, Serialize as rSerialize};
use serde::{self, Deserialize, Serialize};
use sorella_db_databases::{clickhouse, clickhouse::Row};

use super::builder::BuilderInfo;
use crate::{
    db::builder::BuilderInfoRedefined, implement_table_value_codecs_with_zc, mev::MevType,
};

#[derive(Debug, Default, Row, PartialEq, Clone, Serialize, Deserialize, Redefined)]
#[redefined_attr(derive(Debug, PartialEq, Clone, Serialize, rSerialize, rDeserialize, Archive))]
pub struct SearcherInfo {
    #[redefined(same_fields)]
    pub fund: Option<Fund>,
    pub pnl: f64,
    pub total_bribed: f64,
    #[redefined(same_fields)]
    pub mev: Vec<MevType>,
    pub builder: Option<BuilderInfo>,
    pub last_active: u64,
}

impl SearcherInfo {
    pub fn contains_searcher_type(&self, mev_type: MevType) -> bool {
        self.mev.contains(&mev_type)
    }
}

implement_table_value_codecs_with_zc!(SearcherInfoRedefined);

#[derive(
    Debug,
    Default,
    PartialEq,
    Clone,
    Serialize,
    Deserialize,
    rSerialize,
    rDeserialize,
    Archive,
    Copy,
)]
pub enum Fund {
    #[default]
    None,
}

self_convert_redefined!(Fund);
