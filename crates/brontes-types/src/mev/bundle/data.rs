use std::fmt::Debug;

use clickhouse::InsertRow;
use redefined::Redefined;
use reth_primitives::B256;
use rkyv::{Archive, Deserialize as rDeserialize, Serialize as rSerialize};
use serde::{Deserialize, Serialize, Serializer};
use strum::{Display, EnumIter};

use crate::mev::*;
#[allow(unused_imports)]
use crate::{
    display::utils::display_sandwich,
    normalized_actions::{NormalizedBurn, NormalizedLiquidation, NormalizedMint, NormalizedSwap},
    GasDetails,
};

pub struct BundleDataWithRevenue {
    pub revenue: f64,
    pub data:    BundleData,
}

#[derive(Debug, Deserialize, PartialEq, EnumIter, Clone, Display, Redefined)]
#[redefined_attr(derive(Debug, PartialEq, Clone, Serialize, rSerialize, rDeserialize, Archive))]
pub enum BundleData {
    Sandwich(Sandwich),
    AtomicArb(AtomicArb),
    JitSandwich(JitLiquiditySandwich),
    Jit(JitLiquidity),
    CexDex(CexDex),
    Liquidation(Liquidation),
    Unknown,
}

impl Mev for BundleData {
    fn mev_type(&self) -> MevType {
        match self {
            BundleData::Sandwich(m) => m.mev_type(),
            BundleData::AtomicArb(m) => m.mev_type(),
            BundleData::JitSandwich(m) => m.mev_type(),
            BundleData::Jit(m) => m.mev_type(),
            BundleData::CexDex(m) => m.mev_type(),
            BundleData::Liquidation(m) => m.mev_type(),
            BundleData::Unknown => MevType::Unknown,
        }
    }

    fn total_gas_paid(&self) -> u128 {
        match self {
            BundleData::Sandwich(m) => m.total_gas_paid(),
            BundleData::AtomicArb(m) => m.total_gas_paid(),
            BundleData::JitSandwich(m) => m.total_gas_paid(),
            BundleData::Jit(m) => m.total_gas_paid(),
            BundleData::CexDex(m) => m.total_gas_paid(),
            BundleData::Liquidation(m) => m.total_gas_paid(),
            BundleData::Unknown => unimplemented!("calling total_gas_paid() on unknown mev"),
        }
    }

    fn total_priority_fee_paid(&self, base_fee: u128) -> u128 {
        match self {
            BundleData::Sandwich(m) => m.total_priority_fee_paid(base_fee),
            BundleData::AtomicArb(m) => m.total_priority_fee_paid(base_fee),
            BundleData::JitSandwich(m) => m.total_priority_fee_paid(base_fee),
            BundleData::Jit(m) => m.total_priority_fee_paid(base_fee),
            BundleData::CexDex(m) => m.total_priority_fee_paid(base_fee),
            BundleData::Liquidation(m) => m.total_priority_fee_paid(base_fee),
            BundleData::Unknown => {
                unimplemented!("calling total_priority_fee_paid() on unknown mev")
            }
        }
    }

    fn bribe(&self) -> u128 {
        match self {
            BundleData::Sandwich(m) => m.bribe(),
            BundleData::AtomicArb(m) => m.bribe(),
            BundleData::JitSandwich(m) => m.bribe(),
            BundleData::Jit(m) => m.bribe(),
            BundleData::CexDex(m) => m.bribe(),
            BundleData::Liquidation(m) => m.bribe(),
            BundleData::Unknown => unimplemented!("calling bribe() on unknown mev"),
        }
    }

    fn mev_transaction_hashes(&self) -> Vec<B256> {
        match self {
            BundleData::Sandwich(m) => m.mev_transaction_hashes(),
            BundleData::AtomicArb(m) => m.mev_transaction_hashes(),
            BundleData::JitSandwich(m) => m.mev_transaction_hashes(),
            BundleData::Jit(m) => m.mev_transaction_hashes(),
            BundleData::CexDex(m) => m.mev_transaction_hashes(),
            BundleData::Liquidation(m) => m.mev_transaction_hashes(),
            BundleData::Unknown => {
                unimplemented!("calling mev_transaction_hashes() on unknown mev")
            }
        }
    }
}

impl From<Sandwich> for BundleData {
    fn from(value: Sandwich) -> Self {
        Self::Sandwich(value)
    }
}

impl From<AtomicArb> for BundleData {
    fn from(value: AtomicArb) -> Self {
        Self::AtomicArb(value)
    }
}

impl From<JitLiquiditySandwich> for BundleData {
    fn from(value: JitLiquiditySandwich) -> Self {
        Self::JitSandwich(value)
    }
}

impl From<JitLiquidity> for BundleData {
    fn from(value: JitLiquidity) -> Self {
        Self::Jit(value)
    }
}

impl From<CexDex> for BundleData {
    fn from(value: CexDex) -> Self {
        Self::CexDex(value)
    }
}

impl From<Liquidation> for BundleData {
    fn from(value: Liquidation) -> Self {
        Self::Liquidation(value)
    }
}

impl Serialize for BundleData {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            BundleData::Sandwich(sandwich) => sandwich.serialize(serializer),
            BundleData::AtomicArb(backrun) => backrun.serialize(serializer),
            BundleData::JitSandwich(jit_sandwich) => jit_sandwich.serialize(serializer),
            BundleData::Jit(jit) => jit.serialize(serializer),
            BundleData::CexDex(cex_dex) => cex_dex.serialize(serializer),
            BundleData::Liquidation(liquidation) => liquidation.serialize(serializer),
            BundleData::Unknown => {
                unimplemented!("attempted to serialize unknown mev: UNIMPLEMENTED")
            }
        }
    }
}

impl InsertRow for BundleData {
    fn get_column_names(&self) -> &'static [&'static str] {
        match self {
            BundleData::Sandwich(sandwich) => sandwich.get_column_names(),
            BundleData::AtomicArb(backrun) => backrun.get_column_names(),
            BundleData::JitSandwich(jit_sandwich) => jit_sandwich.get_column_names(),
            BundleData::Jit(jit) => jit.get_column_names(),
            BundleData::CexDex(cex_dex) => cex_dex.get_column_names(),
            BundleData::Liquidation(liquidation) => liquidation.get_column_names(),
            BundleData::Unknown => {
                unimplemented!("attempted to inserted unknown mev into clickhouse: UNIMPLEMENTED")
            }
        }
    }
}
