use alloy_primitives::Address;
use clickhouse::Row;
use redefined::{self_convert_redefined, Redefined};
use rkyv::{Archive, Deserialize as rDeserialize, Serialize as rSerialize};
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use strum::Display;

use crate::{
    db::redefined_types::primitives::AddressRedefined,
    implement_table_value_codecs_with_zc,
    mev::{BundleHeader, MevType},
    serde_utils::{addresss, option_addresss},
};

#[derive(Debug, Default, Row, PartialEq, Clone, Serialize, Deserialize, Redefined)]
#[redefined_attr(derive(Debug, PartialEq, Clone, Serialize, rSerialize, rDeserialize, Archive))]
pub struct SearcherInfo {
    #[redefined(same_fields)]
    #[serde(default)]
    pub fund:    Fund,
    #[redefined(same_fields)]
    #[serde(default)]
    pub mev:     Vec<MevType>,
    /// If the searcher is vertically integrated, this will contain the
    /// corresponding builder's information.
    #[serde(with = "option_addresss")]
    #[serde(default)]
    pub builder: Option<Address>,
}

impl SearcherInfo {
    pub fn contains_searcher_type(&self, mev_type: MevType) -> bool {
        self.mev.contains(&mev_type)
    }

    pub fn merge(&mut self, other: SearcherInfo) {
        self.fund = other.fund;
        for mev_type in other.mev.into_iter() {
            if !self.contains_searcher_type(mev_type) {
                self.mev.push(mev_type);
            }
        }
        self.builder = other.builder.or(self.builder.take());
    }
}

implement_table_value_codecs_with_zc!(SearcherInfoRedefined);

/// Aggregated searcher statistics, updated once the brontes analytics are run.
/// The key is the mev contract address.
#[derive(Debug, Default, Row, PartialEq, Clone, Serialize, Deserialize, Redefined)]
#[redefined_attr(derive(Debug, PartialEq, Clone, Serialize, rSerialize, rDeserialize, Archive))]
pub struct SearcherStats {
    pub pnl:          f64,
    pub total_bribed: f64,
    pub bundle_count: u64,
    /// The block number of the most recent bundle involving this searcher.
    pub last_active:  u64,
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

#[derive(Debug, Default, Row, PartialEq, Clone, Serialize, Deserialize)]
pub struct SearcherStatsWithAddress {
    #[serde(with = "addresss")]
    pub address:      Address,
    pub pnl:          f64,
    pub total_bribed: f64,
    pub bundle_count: u64,
    pub last_active:  u64,
}

impl SearcherStatsWithAddress {
    pub fn new_with_address(address: Address, stats: SearcherStats) -> Self {
        Self {
            address,
            pnl: stats.pnl,
            total_bribed: stats.total_bribed,
            bundle_count: stats.bundle_count,
            last_active: stats.last_active,
        }
    }
}

#[derive(
    Debug, Default, Display, PartialEq, Eq, Clone, rSerialize, rDeserialize, Archive, Copy,
)]
pub enum Fund {
    #[default]
    None,
    SymbolicCapitalPartners,
    Wintermute,
    JaneStreet,
    JumpTrading,
    FlowTraders,
    TokkaLabs,
    EthBuilder,
    ICANHAZBLOCK,
}

impl From<String> for Fund {
    fn from(value: String) -> Self {
        match value.as_str() {
            "SymbolicCapitalPartners" => Self::SymbolicCapitalPartners,
            "Wintermute" => Self::Wintermute,
            "JaneStreet" => Self::JaneStreet,
            "JumpTrading" => Self::JumpTrading,
            "FlowTraders" => Self::FlowTraders,
            "TokkaLabs" => Self::TokkaLabs,
            "EthBuilder" => Self::EthBuilder,
            "ICANHAZBLOCK" => Self::ICANHAZBLOCK,
            _ => Self::None,
        }
    }
}

impl Serialize for Fund {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let fund_str = format!("{}", self);

        Serialize::serialize(&fund_str, serializer)
    }
}

impl<'de> Deserialize<'de> for Fund {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let fund: String = Deserialize::deserialize(deserializer)?;

        Ok(fund.into())
    }
}

self_convert_redefined!(Fund);

#[derive(Debug, Row, PartialEq, Clone, Serialize, Deserialize)]
pub struct JoinedSearcherInfo {
    #[serde(with = "addresss")]
    pub address:         Address,
    pub fund:            Fund,
    pub mev:             Vec<MevType>,
    #[serde(with = "option_addresss")]
    pub builder:         Option<Address>,
    pub eoa_or_contract: SearcherEoaContract,
}

impl JoinedSearcherInfo {
    pub fn new_eoa(address: Address, info: SearcherInfo) -> Self {
        Self {
            address,
            fund: info.fund,
            mev: info.mev,
            builder: info.builder,
            eoa_or_contract: SearcherEoaContract::EOA,
        }
    }

    pub fn new_contract(address: Address, info: SearcherInfo) -> Self {
        Self {
            address,
            fund: info.fund,
            mev: info.mev,
            builder: info.builder,
            eoa_or_contract: SearcherEoaContract::Contract,
        }
    }
}

#[derive(Debug, PartialEq, Clone, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum SearcherEoaContract {
    EOA      = 0,
    Contract = 1,
}
