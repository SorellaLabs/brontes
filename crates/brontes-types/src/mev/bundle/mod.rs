pub mod data;
pub mod header;
use std::fmt::{self, Debug};

use alloy_primitives::Address;
use clap::ValueEnum;
use clickhouse::Row;
pub use data::*;
use dyn_clone::DynClone;
pub use header::*;
use redefined::{self_convert_redefined, Redefined};
use reth_primitives::B256;
use rkyv::{Archive, Deserialize as rDeserialize, Serialize as rSerialize};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use strum::{Display, EnumIter};

use crate::display::utils::*;
#[allow(unused_imports)]
use crate::{
    display::utils::{display_cex_dex, display_sandwich},
    normalized_actions::{NormalizedBurn, NormalizedLiquidation, NormalizedMint, NormalizedSwap},
    GasDetails,
};

#[serde_as]
#[derive(Debug, Serialize, Deserialize, PartialEq, Row, Clone, Redefined)]
#[redefined_attr(derive(Debug, PartialEq, Clone, Serialize, rSerialize, rDeserialize, Archive))]
pub struct Bundle {
    pub header: BundleHeader,
    pub data:   BundleData,
}

impl Bundle {
    pub fn get_searcher_contract(&self) -> Option<Address> {
        self.header.mev_contract
    }

    pub fn mev_type(&self) -> MevType {
        self.header.mev_type
    }
}

impl fmt::Display for Bundle {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.header.mev_type {
            MevType::Sandwich => display_sandwich(self, f)?,
            MevType::CexDex => display_cex_dex(self, f)?,
            MevType::Jit => display_jit_liquidity(self, f)?,
            MevType::AtomicArb => display_atomic_backrun(self, f)?,
            MevType::Liquidation => display_liquidation(self, f)?,
            MevType::JitSandwich => display_jit_liquidity_sandwich(self, f)?,
            MevType::Unknown => display_searcher_tx(self, f)?,
        }

        Ok(())
    }
}

#[derive(
    Debug,
    PartialEq,
    Eq,
    Hash,
    EnumIter,
    rkyv::Serialize,
    rkyv::Deserialize,
    rkyv::Archive,
    Clone,
    Copy,
    Default,
    Display,
    ValueEnum,
)]
pub enum MevType {
    CexDex,
    Sandwich,
    Jit,
    JitSandwich,
    Liquidation,
    AtomicArb,
    #[default]
    Unknown,
}

impl MevType {
    pub fn use_cex_pricing_for_deltas(&self) -> bool {
        match self {
            MevType::Sandwich
            | MevType::JitSandwich
            | MevType::Jit
            | MevType::AtomicArb
            | MevType::Liquidation
            | MevType::Unknown => false,
            MevType::CexDex => true,
        }
    }
}

impl From<String> for MevType {
    fn from(value: String) -> Self {
        let val = value.as_str();

        match val {
            "CexDex" => MevType::CexDex,
            "Sandwich" => MevType::Sandwich,
            "Jit" => MevType::Jit,
            "Liquidation" => MevType::Liquidation,
            "JitSandwich" => MevType::JitSandwich,
            "AtomicArb" => MevType::AtomicArb,
            _ => MevType::Unknown,
        }
    }
}

impl Serialize for MevType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mev_type = format!("{}", self);

        Serialize::serialize(&mev_type, serializer)
    }
}

impl<'de> Deserialize<'de> for MevType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let mev_type: String = Deserialize::deserialize(deserializer)?;

        Ok(mev_type.into())
    }
}

self_convert_redefined!(MevType);

pub trait Mev: erased_serde::Serialize + Send + Sync + Debug + 'static + DynClone {
    fn mev_type(&self) -> MevType;

    /// The total amount of gas paid by the bundle in wei
    /// This includes the coinbase transfer, if any
    fn total_gas_paid(&self) -> u128;

    /// The priority fee paid by the bundle in wei
    /// Effective gas - base fee * gas used
    fn total_priority_fee_paid(&self, base_fee: u128) -> u128;

    fn bribe(&self) -> u128;
    fn mev_transaction_hashes(&self) -> Vec<B256>;
}

dyn_clone::clone_trait_object!(Mev);
