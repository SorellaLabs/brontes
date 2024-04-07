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
    pub fund:          Fund,
    #[redefined(same_fields)]
    #[serde(default)]
    pub mev_count:     MevCount,
    #[redefined(same_fields)]
    #[serde(default)]
    pub pnl:           TollByType,
    #[redefined(same_fields)]
    #[serde(default)]
    pub gas_bids:      TollByType,
    /// If the searcher is vertically integrated, this will contain the
    /// corresponding builder's information.
    #[serde(with = "option_addresss")]
    #[serde(default)]
    pub builder:       Option<Address>,
    #[redefined(same_fields)]
    #[serde(default)]
    #[serde(rename = "mev_types")]
    pub config_labels: Vec<MevType>,
}

impl SearcherInfo {
    pub fn is_searcher_of_type(&self, mev_type: MevType) -> bool {
        self.get_bundle_count_for_type(mev_type).is_some()
    }

    pub fn is_searcher_of_type_with_threshold(&self, mev_type: MevType, threshold: u64) -> bool {
        self.get_bundle_count_for_type(mev_type)
            .map(|count| count >= threshold)
            .unwrap_or(false)
    }

    pub fn get_bundle_count_for_type(&self, mev_type: MevType) -> Option<u64> {
        match mev_type {
            MevType::Sandwich => self.mev_count.sandwich_count,
            MevType::CexDex => self.mev_count.cex_dex_count,
            MevType::Jit => self.mev_count.jit_count,
            MevType::JitSandwich => self.mev_count.jit_sandwich_count,
            MevType::AtomicArb => self.mev_count.atomic_backrun_count,
            MevType::Liquidation => self.mev_count.liquidation_count,
            MevType::SearcherTx => self.mev_count.searcher_tx_count,
            MevType::Unknown => None,
        }
    }

    pub fn is_labelled_searcher_of_type(&self, mev_type: MevType) -> bool {
        self.config_labels.contains(&mev_type)
    }

    pub fn merge(&mut self, other: SearcherInfo) {
        self.fund = other.fund;

        for mev_type in other.config_labels.into_iter() {
            if !self.config_labels.contains(&mev_type) {
                self.config_labels.push(mev_type);
            }
        }

        if other.mev_count.bundle_count > 0 {
            self.mev_count = other.mev_count;
        }
        self.builder = other.builder.or(self.builder.take());
    }

    pub fn describe(&self) -> String {
        let mut parts: Vec<String> = Vec::new();

        if self.builder.is_some() {
            parts.push("Vertically Integrated".to_string());
        }

        if let Fund::None = self.fund {
        } else {
            parts.push(format!("{}", self.fund));
        }

        let mev_types: Vec<String> = vec![
            ("Sandwich", self.mev_count.sandwich_count, self.pnl.sandwich, self.gas_bids.sandwich),
            ("CexDex", self.mev_count.cex_dex_count, self.pnl.cex_dex, self.gas_bids.cex_dex),
            ("Jit", self.mev_count.jit_count, self.pnl.jit, self.gas_bids.jit),
            (
                "JitSandwich",
                self.mev_count.jit_sandwich_count,
                self.pnl.jit_sandwich,
                self.gas_bids.jit_sandwich,
            ),
            (
                "AtomicArb",
                self.mev_count.atomic_backrun_count,
                self.pnl.atomic_backrun,
                self.gas_bids.atomic_backrun,
            ),
            (
                "Liquidation",
                self.mev_count.liquidation_count,
                self.pnl.liquidation,
                self.gas_bids.liquidation,
            ),
        ]
        .into_iter()
        .filter_map(|(mev_type, count, pnl, gas_bid)| {
            if let (Some(count), Some(pnl), Some(gas_paid)) = (count, pnl, gas_bid) {
                if count > 10 && pnl > 0.0 && gas_paid > 10.0 {
                    Some(mev_type.to_string())
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect();

        if !mev_types.is_empty() {
            parts.push(format!("{} MEV bot", mev_types.join(" & ")));
        } else {
            parts.push("MEV bot".to_string());
        }

        parts.join(" ")
    }

    pub fn update_with_bundle(&mut self, header: &BundleHeader) {
        self.pnl.account_pnl(header);
        self.mev_count.increment_count(header.mev_type);
        self.gas_bids.account_gas(header);
    }
}

implement_table_value_codecs_with_zc!(SearcherInfoRedefined);

#[serde_as]
#[derive(
    Debug,
    Deserialize,
    PartialEq,
    Serialize,
    Row,
    Clone,
    Default,
    rkyv::Serialize,
    rDeserialize,
    Archive,
)]
pub struct TollByType {
    pub total:          f64,
    pub sandwich:       Option<f64>,
    pub cex_dex:        Option<f64>,
    pub jit:            Option<f64>,
    pub jit_sandwich:   Option<f64>,
    pub atomic_backrun: Option<f64>,
    pub liquidation:    Option<f64>,
    pub searcher_tx:    Option<f64>,
}

self_convert_redefined!(TollByType);

impl TollByType {
    pub fn account_pnl(&mut self, header: &BundleHeader) {
        self.total += header.profit_usd;
        match header.mev_type {
            MevType::CexDex => {
                self.cex_dex = Some(self.cex_dex.unwrap_or_default().add(header.profit_usd))
            }
            MevType::Sandwich => {
                self.sandwich = Some(self.sandwich.unwrap_or_default().add(header.profit_usd))
            }
            MevType::AtomicArb => {
                self.atomic_backrun = Some(
                    self.atomic_backrun
                        .unwrap_or_default()
                        .add(header.profit_usd),
                )
            }
            MevType::Jit => self.jit = Some(self.jit.unwrap_or_default().add(header.profit_usd)),
            MevType::JitSandwich => {
                self.jit_sandwich =
                    Some(self.jit_sandwich.unwrap_or_default().add(header.profit_usd))
            }
            MevType::Liquidation => {
                self.liquidation = Some(self.liquidation.unwrap_or_default().add(header.profit_usd))
            }
            MevType::SearcherTx => {
                self.searcher_tx = Some(self.searcher_tx.unwrap_or_default().add(header.profit_usd))
            }
            _ => (),
        }
    }

    pub fn account_gas(&mut self, header: &BundleHeader) {
        self.total += header.bribe_usd;
        match header.mev_type {
            MevType::CexDex => {
                self.cex_dex = Some(self.cex_dex.unwrap_or_default().add(header.bribe_usd))
            }
            MevType::Sandwich => {
                self.sandwich = Some(self.sandwich.unwrap_or_default().add(header.bribe_usd))
            }
            MevType::AtomicArb => {
                self.atomic_backrun = Some(
                    self.atomic_backrun
                        .unwrap_or_default()
                        .add(header.bribe_usd),
                )
            }
            MevType::Jit => self.jit = Some(self.jit.unwrap_or_default().add(header.bribe_usd)),
            MevType::JitSandwich => {
                self.jit_sandwich =
                    Some(self.jit_sandwich.unwrap_or_default().add(header.bribe_usd))
            }
            MevType::Liquidation => {
                self.liquidation = Some(self.liquidation.unwrap_or_default().add(header.bribe_usd))
            }
            MevType::SearcherTx => {
                self.searcher_tx = Some(self.searcher_tx.unwrap_or_default().add(header.bribe_usd))
            }
            _ => (),
        }
    }
}

#[derive(
    Debug,
    Default,
    Display,
    PartialEq,
    Eq,
    Clone,
    rSerialize,
    rDeserialize,
    Archive,
    Copy,
    AsRefStr,
    PartialOrd,
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
            "SymbolicCapitalPartners" => Self::SymbolicCapitalPartners,
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
    pub address:         Address,
    pub eoa_or_contract: SearcherEoaContract,
    pub fund:            Fund,
    pub config_labels:   Vec<MevType>,
    #[serde(with = "option_addresss")]
    pub builder:         Option<Address>,
    pub mev:             MevCount,
    pub pnl:             TollByType,
    pub gas_bids:        TollByType,
}

impl JoinedSearcherInfo {
    pub fn new_eoa(address: Address, info: SearcherInfo) -> Self {
        Self {
            address,
            eoa_or_contract: SearcherEoaContract::EOA,
            fund: info.fund,
            config_labels: info.config_labels,
            mev: info.mev_count,
            pnl: info.pnl,
            gas_bids: info.gas_bids,
            builder: info.builder,
        }
    }

    pub fn new_contract(address: Address, info: SearcherInfo) -> Self {
        Self {
            address,
            eoa_or_contract: SearcherEoaContract::Contract,
            fund: info.fund,
            config_labels: info.config_labels,
            mev: info.mev_count,
            pnl: info.pnl,
            gas_bids: info.gas_bids,
            builder: info.builder,
        }
    }
}

#[derive(Debug, PartialEq, Clone, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum SearcherEoaContract {
    EOA      = 0,
    Contract = 1,
}
