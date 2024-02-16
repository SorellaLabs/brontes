use clickhouse::{self, Row};
use redefined::{self_convert_redefined, Redefined};
use rkyv::{Archive, Deserialize as rDeserialize, Serialize as rSerialize};
use serde::{self, Deserialize, Serialize};

use super::builder::BuilderInfo;
use crate::{
    db::builder::BuilderInfoRedefined,
    implement_table_value_codecs_with_zc,
    mev::{BundleHeader, MevType},
};

#[derive(Debug, Default, Row, PartialEq, Clone, Serialize, Deserialize, Redefined)]
#[redefined_attr(derive(Debug, PartialEq, Clone, Serialize, rSerialize, rDeserialize, Archive))]
pub struct SearcherInfo {
    #[redefined(same_fields)]
    pub fund: Option<Fund>,
    #[redefined(same_fields)]
    pub mev: Vec<MevType>,
    /// If the searcher is vertically integrated, this will contain the corresponding builder's information.
    pub builder: Option<BuilderInfo>,
}

impl SearcherInfo {
    pub fn contains_searcher_type(&self, mev_type: MevType) -> bool {
        self.mev.contains(&mev_type)
    }
}

implement_table_value_codecs_with_zc!(SearcherInfoRedefined);

/// Aggregated searcher statistics, updated once the brontes analytics are run.
#[derive(Debug, Default, Row, PartialEq, Clone, Serialize, Deserialize, Redefined)]
#[redefined_attr(derive(Debug, PartialEq, Clone, Serialize, rSerialize, rDeserialize, Archive))]
pub struct SearcherStats {
    pub pnl: f64,
    pub total_bribed: f64,
    pub bundle_count: u64,
    /// The block number of the most recent bundle involving this searcher.
    pub last_active: u64,
}

impl SearcherStats {
    pub fn update_with_bundle(&mut self, header: &BundleHeader) {
        self.pnl += header.profit_usd;
        self.total_bribed += header.bribe_usd;
        self.bundle_count += 1;
        self.last_active = header.block_number;
    }
}

implement_table_value_codecs_with_zc!(SearcherStatsRedefined);

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
    SymbolicCapitalPartners,
    Wintermute,
    JaneStreet,
}

self_convert_redefined!(Fund);
