use std::{fmt, ops::Add};

use alloy_primitives::Address;
use clickhouse::Row;
use redefined::{self_convert_redefined, Redefined};
use rkyv::{Archive, Deserialize as rDeserialize, Serialize as rSerialize};
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use serde_with::serde_as;
use strum::AsRefStr;

use crate::{
    db::redefined_types::primitives::AddressRedefined,
    implement_table_value_codecs_with_zc,
    mev::{BundleHeader, MevCount, MevType},
    serde_utils::{addresss, option_addresss, vec_address},
    FastHashMap,
};

#[derive(Debug, Default, Row, PartialEq, Clone, Serialize, Deserialize, Redefined)]
#[redefined_attr(derive(Debug, PartialEq, Clone, Serialize, rSerialize, rDeserialize, Archive))]
pub struct SearcherInfo {
    #[serde(default)]
    pub name:              Option<String>,
    #[redefined(same_fields)]
    #[serde(default)]
    pub fund:              Fund,
    #[redefined(same_fields)]
    #[serde(default)]
    /// Bundle count by mev type
    pub mev_count:         MevCount,
    #[redefined(same_fields)]
    #[serde(default)]
    /// Profit and loss by mev type
    pub pnl:               TollByType,
    #[redefined(same_fields)]
    #[serde(default)]
    /// Gas bids by mev type
    pub gas_bids:          TollByType,
    /// If the searcher is vertically integrated, this will contain the
    /// corresponding builder's information.
    #[serde(with = "option_addresss")]
    #[serde(default)]
    pub builder:           Option<Address>,
    #[redefined(same_fields)]
    #[serde(default)]
    #[serde(rename = "mev_types")]
    pub config_labels:     Vec<MevType>,
    #[serde(with = "vec_address")]
    #[serde(default)]
    pub sibling_searchers: Vec<Address>,
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

    pub fn get_sibling_searchers(&self) -> &Vec<Address> {
        self.sibling_searchers.as_ref()
    }

    pub fn get_bundle_count_for_type(&self, mev_type: MevType) -> Option<u64> {
        match mev_type {
            MevType::CexDexTrades => self.mev_count.cex_dex_trade_count,
            MevType::CexDexQuotes => self.mev_count.cex_dex_quote_count,
            MevType::CexDexRfq => self.mev_count.cex_dex_rfq_count,
            MevType::JitCexDex => self.mev_count.jit_cex_dex_count,
            MevType::Sandwich => self.mev_count.sandwich_count,
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
        self.name = other.name;

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

        self.sibling_searchers = other.sibling_searchers;
    }

    pub fn describe(&self) -> String {
        if self.name.is_some() {
            return self.name.clone().unwrap()
        }
        let mut parts: Vec<String> = Vec::new();

        if let Fund::None = self.fund {
        } else {
            parts.push(self.fund.to_string());
        }

        let mev_type: Option<String> = vec![
            ("Sandwich", self.mev_count.sandwich_count, self.pnl.sandwich, self.gas_bids.sandwich),
            (
                "CexDexTrades",
                self.mev_count.cex_dex_trade_count,
                self.pnl.cex_dex_trades,
                self.gas_bids.cex_dex_quotes,
            ),
            (
                "CexDexQuotes",
                self.mev_count.cex_dex_quote_count,
                self.pnl.cex_dex_quotes,
                self.gas_bids.cex_dex_trades,
            ),
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
                    Some((mev_type, count))
                } else {
                    None
                }
            } else {
                None
            }
        })
        .max_by_key(|(_, count)| *count)
        .map(|(mev_type, _)| mev_type.to_string());

        if mev_type.is_some() {
            parts.push(format!("{} bot", mev_type.unwrap()));
        }
        parts.join(" ")
    }

    pub fn update_with_bundle(&mut self, header: &BundleHeader) {
        self.pnl.account_pnl(header);
        self.mev_count.increment_count(header.mev_type);
        self.gas_bids.account_gas(header);
    }

    /// Uses elementary scoring to infer the most likely bot type. This is only
    /// used to improve error logs when we need context on the type of bot
    /// that is causing an error.
    pub fn infer_mev_bot_type(&self) -> Option<MevType> {
        let type_scores: FastHashMap<MevType, (u64, f64, f64)> = [
            (
                MevType::Sandwich,
                (self.mev_count.sandwich_count, self.gas_bids.sandwich, self.pnl.sandwich),
            ),
            (
                MevType::CexDexTrades,
                (
                    self.mev_count.cex_dex_trade_count,
                    self.gas_bids.cex_dex_trades,
                    self.pnl.cex_dex_trades,
                ),
            ),
            (
                MevType::CexDexQuotes,
                (
                    self.mev_count.cex_dex_quote_count,
                    self.gas_bids.cex_dex_quotes,
                    self.pnl.cex_dex_quotes,
                ),
            ),
            (MevType::Jit, (self.mev_count.jit_count, self.gas_bids.jit, self.pnl.jit)),
            (
                MevType::JitSandwich,
                (
                    self.mev_count.jit_sandwich_count,
                    self.gas_bids.jit_sandwich,
                    self.pnl.jit_sandwich,
                ),
            ),
            (
                MevType::AtomicArb,
                (
                    self.mev_count.atomic_backrun_count,
                    self.gas_bids.atomic_backrun,
                    self.pnl.atomic_backrun,
                ),
            ),
            (
                MevType::Liquidation,
                (self.mev_count.liquidation_count, self.gas_bids.liquidation, self.pnl.liquidation),
            ),
        ]
        .into_iter()
        .filter_map(|(mev_type, (count, gas, profit))| Some((mev_type, (count?, gas?, profit?))))
        .collect();

        if type_scores.is_empty() {
            return None;
        }

        let totals = type_scores
            .values()
            .fold((0, 0.0, 0.0), |acc, &(count, gas, profit)| {
                (acc.0 + count, acc.1 + gas, acc.2 + profit)
            });

        type_scores
            .into_iter()
            .map(|(mev_type, (count, gas, profit))| {
                let score = if totals.0 > 0 && totals.1 > 0.0 && totals.2 > 0.0 {
                    0.5 * (count as f64 / totals.0 as f64)
                        + 0.3 * (gas / totals.1)
                        + 0.2 * (profit / totals.2)
                } else {
                    0.0
                };
                (mev_type, score)
            })
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(mev_type, _)| mev_type)
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
    pub cex_dex_quotes: Option<f64>,
    pub cex_dex_trades: Option<f64>,
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
            MevType::CexDexTrades => {
                self.cex_dex_trades = Some(
                    self.cex_dex_trades
                        .unwrap_or_default()
                        .add(header.profit_usd),
                )
            }
            MevType::CexDexQuotes => {
                self.cex_dex_quotes = Some(
                    self.cex_dex_quotes
                        .unwrap_or_default()
                        .add(header.profit_usd),
                )
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
            MevType::CexDexQuotes => {
                self.cex_dex_quotes = Some(
                    self.cex_dex_quotes
                        .unwrap_or_default()
                        .add(header.bribe_usd),
                )
            }
            MevType::CexDexTrades => {
                self.cex_dex_trades = Some(
                    self.cex_dex_trades
                        .unwrap_or_default()
                        .add(header.bribe_usd),
                )
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
    PartialEq,
    Eq,
    Clone,
    rSerialize,
    rDeserialize,
    Archive,
    Copy,
    AsRefStr,
    PartialOrd,
    Hash,
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

impl fmt::Display for Fund {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Fund::None => "None",
                Fund::SymbolicCapitalPartners => "SCP",
                Fund::Wintermute => "Wintermute",
                Fund::JaneStreet => "Jane Street",
                Fund::JumpTrading => "Jump Trading",
                Fund::Kronos => "Kronos",
                Fund::FlowTraders => "Flow Traders",
                Fund::TokkaLabs => "Tokka Labs",
                Fund::EthBuilder => "Eth Builder",
                Fund::ICANHAZBLOCK => "I CAN HAZ BLOCK",
            }
        )
    }
}

impl Fund {
    pub fn is_none(&self) -> bool {
        matches!(self, Fund::None)
    }
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
