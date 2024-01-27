pub mod data;
pub mod header;

use std::fmt::{self, Debug};



pub use data::*;
use dyn_clone::DynClone;
pub use header::*;


use reth_primitives::B256;
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use serde_with::serde_as;
use sorella_db_databases::{
    clickhouse,
    clickhouse::{InsertRow, Row},
};
use strum::{Display, EnumIter};

#[allow(unused_imports)]
use crate::{
    display::utils::{display_sandwich, print_mev_type_header},
    normalized_actions::{NormalizedBurn, NormalizedLiquidation, NormalizedMint, NormalizedSwap},
    serde_primitives::vec_fixed_string,
    GasDetails,
};

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Row, Clone, Default)]
pub struct Bundle {
    pub header: BundleHeader,
    pub data:   BundleData,
}

impl fmt::Display for Bundle {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.header.mev_type {
            MevType::Sandwich => display_sandwich(self, f)?,
            _ => unimplemented!(),
        }

        Ok(())
    }
}

#[derive(
    Debug,
    Serialize_repr,
    Deserialize_repr,
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
)]
#[repr(u8)]
#[allow(non_camel_case_types)]
#[serde(rename_all = "lowercase")]
pub enum MevType {
    Sandwich    = 1,
    Backrun     = 5,
    #[serde(rename = "jit_sandwich")]
    JitSandwich = 3,
    Jit         = 2,
    #[serde(rename = "cex_dex")]
    CexDex      = 0,
    Liquidation = 4,
    #[default]
    Unknown     = 6,
}

pub trait Mev:
    InsertRow + erased_serde::Serialize + Send + Sync + Debug + 'static + DynClone
{
    fn mev_type(&self) -> MevType;
    // the amount of gas they paid in wei
    fn priority_fee_paid(&self) -> u128;
    fn bribe(&self) -> u128;
    fn mev_transaction_hashes(&self) -> Vec<B256>;
}

dyn_clone::clone_trait_object!(Mev);
