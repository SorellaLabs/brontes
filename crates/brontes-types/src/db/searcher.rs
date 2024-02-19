use alloy_primitives::Address;
use clickhouse::{self, Row};
use redefined::{self_convert_redefined, Redefined};
use rkyv::{Archive, Deserialize as rDeserialize, Serialize as rSerialize};
use serde::{self, Deserialize, Serialize};

use crate::{
    db::redefined_types::primitives::AddressRedefined,
    implement_table_value_codecs_with_zc,
    mev::{BundleHeader, MevType},
    serde_utils::option_addresss,
};

#[derive(Debug, Default, Row, PartialEq, Clone, Serialize, Deserialize, Redefined)]
#[redefined_attr(derive(Debug, PartialEq, Clone, Serialize, rSerialize, rDeserialize, Archive))]
pub struct SearcherInfo {
    #[redefined(same_fields)]
    #[serde(default)]
    pub fund: Option<Fund>,
    #[redefined(same_fields)]
    #[serde(default)]
    pub mev: Vec<MevType>,
    /// If the searcher is vertically integrated, this will contain the corresponding builder's information.
    #[serde(with = "option_addresss")]
    #[serde(default)]
    pub builder: Option<Address>,
}

impl SearcherInfo {
    pub fn contains_searcher_type(&self, mev_type: MevType) -> bool {
        self.mev.contains(&mev_type)
    }

    pub fn merge(&mut self, other: SearcherInfo) {
        self.fund = other.fund.or(self.fund.take());
        self.mev.extend(other.mev);
        self.builder = other.builder.or(self.builder.take());
    }
}

implement_table_value_codecs_with_zc!(SearcherInfoRedefined);

/// Aggregated searcher statistics, updated once the brontes analytics are run. The key is the mev contract address.
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
    Eq,
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
    FlowTraders,
}

self_convert_redefined!(Fund);
