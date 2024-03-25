use std::ops::Add;

use alloy_primitives::Address;
use clickhouse::Row;
use redefined::{self_convert_redefined, Redefined};
use rkyv::{Archive, Deserialize as rDeserialize, Serialize as rSerialize};
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use serde_with::serde_as;
use strum::{AsRefStr, Display};

use crate::{
    db::redefined_types::primitives::AddressRedefined,
    implement_table_value_codecs_with_zc,
    mev::{BundleHeader, MevCount, MevType},
    serde_utils::{addresss, option_addresss},
};

#[derive(Debug, Default, Row, PartialEq, Clone, Serialize, Deserialize, Redefined)]
#[redefined_attr(derive(Debug, PartialEq, Clone, Serialize, rSerialize, rDeserialize, Archive))]
pub struct SearcherInfo {
    #[redefined(same_fields)]
    #[serde(default)]
    pub fund: Fund,
    #[redefined(same_fields)]
    #[serde(default)]
    pub mev: Vec<MevType>,
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

    pub fn describe(&self) -> String {
        let mut parts: Vec<String> = Vec::new();

        if self.builder.is_some() {
            parts.push("Vertically Integrated".into());
        }

        match self.fund {
            Fund::None => (),
            fund => parts.push(format!("{}", fund)),
        }

        if !self.mev.is_empty() {
            let mev_types: Vec<String> = self.mev.iter().map(|mev| format!("{:?}", mev)).collect();
            let mev_part = mev_types.join(" & ");
            parts.push(mev_part + " MEV bot");
        } else {
            parts.push("MEV bot".into());
        }

        parts.join(" ")
    }
}

implement_table_value_codecs_with_zc!(SearcherInfoRedefined);

/// Aggregated searcher statistics, updated once the brontes analytics are run.
/// The key is the mev contract address.
#[derive(Debug, Default, Row, PartialEq, Clone, Serialize, Deserialize, Redefined)]
#[redefined_attr(derive(Debug, PartialEq, Clone, Serialize, rSerialize, rDeserialize, Archive))]
pub struct SearcherStats {
    #[redefined(same_fields)]
    pub pnl: ProfitByType,
    #[redefined(same_fields)]
    pub total_bribed: ProfitByType,
    #[redefined(same_fields)]
    pub bundle_count: MevCount,
    /// The block number of the most recent bundle involving this searcher.
    pub last_active: u64,
}

//TODO: Cleanup
impl SearcherStats {
    pub fn update_with_bundle(&mut self, header: &BundleHeader) {
        self.pnl.account_by_type(header);
        self.total_bribed.account_by_type(header);
        self.bundle_count.increment_count(&header.mev_type);
        self.last_active = header.block_number;
    }
}

implement_table_value_codecs_with_zc!(SearcherStatsRedefined);

#[derive(Debug, Default, Row, PartialEq, Clone, Serialize, Deserialize)]
pub struct SearcherStatsWithAddress {
    #[serde(with = "addresss")]
    pub address: Address,
    pub pnl: ProfitByType,
    pub total_bribed: ProfitByType,
    pub bundle_count: MevCount,
    pub last_active: u64,
}
#[serde_as]
#[derive(
    Debug, Deserialize, PartialEq, Serialize, Row, Clone, Default, rSerialize, rDeserialize, Archive,
)]
pub struct ProfitByType {
    pub total_pnl: f64,
    pub sandwich_pnl: Option<f64>,
    pub cex_dex_pnl: Option<f64>,
    pub jit_pnl: Option<f64>,
    pub jit_sandwich_pnl: Option<f64>,
    pub atomic_backrun_pnl: Option<f64>,
    pub liquidation_pnl: Option<f64>,
    pub searcher_tx_pnl: Option<f64>,
}

self_convert_redefined!(ProfitByType);

impl ProfitByType {
    pub fn account_by_type(&mut self, header: &BundleHeader) {
        self.total_pnl += header.profit_usd;
        match header.mev_type {
            MevType::CexDex => {
                self.cex_dex_pnl = Some(self.cex_dex_pnl.unwrap_or_default().add(header.profit_usd))
            }
            MevType::Sandwich => {
                self.sandwich_pnl = Some(self.sandwich_pnl.unwrap_or_default().add(header.profit_usd))
            }
            MevType::AtomicArb => {
                self.atomic_backrun_pnl = Some(
                    self.atomic_backrun_pnl
                        .unwrap_or_default()
                        .add(header.profit_usd),
                )
            }
            MevType::Jit => {
                self.jit_pnl = Some(self.jit_pnl.unwrap_or_default().add(header.profit_usd))
            }
            MevType::JitSandwich => {
                self.jit_sandwich_pnl = Some(
                    self.jit_sandwich_pnl
                        .unwrap_or_default()
                        .add(header.profit_usd),
                )
            }
            MevType::Liquidation => {
                self.liquidation_pnl = Some(
                    self.liquidation_pnl
                        .unwrap_or_default()
                        .add(header.profit_usd),
                )
            }
            MevType::SearcherTx => {

                //TODO: For now we don't account searcher tx pnl
                /*                 self.searcher_tx_pnl = Some(
                    self.searcher_tx_pnl
                        .unwrap_or_default()
                        .add(header.profit_usd),
                ) */
            }
            _ => (),
        }
    }
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
    Debug, Default, Display, PartialEq, Eq, Clone, rSerialize, rDeserialize, Archive, Copy, AsRefStr,
)]
pub enum Fund {
    #[default]
    None,
    SymbolicCapitalPartners,
    Wintermute,
    JaneStreet,
    JumpTrading,
    Kronos,
    FlowTraders,
    TokkaLabs,
    EthBuilder,
    ICANHAZBLOCK,
}

impl From<String> for Fund {
    fn from(value: String) -> Self {
        match value.as_str() {
            "Symbolic Capital Partners" => Self::SymbolicCapitalPartners,
            "Wintermute" => Self::Wintermute,
            "Jane Street" => Self::JaneStreet,
            "Jump Trading" => Self::JumpTrading,
            "Flow Traders" => Self::FlowTraders,
            "Tokka Labs" => Self::TokkaLabs,
            "Kronos Research" => Self::Kronos,
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
    pub address: Address,
    pub fund: Fund,
    pub mev: Vec<MevType>,
    #[serde(with = "option_addresss")]
    pub builder: Option<Address>,
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
    EOA = 0,
    Contract = 1,
}
