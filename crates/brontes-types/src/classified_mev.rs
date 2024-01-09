use std::{any::Any, fmt::Debug};

use alloy_primitives::{Address, U256};
use dyn_clone::DynClone;
use reth_primitives::B256;
use serde::{
    ser::{SerializeStruct, SerializeTuple},
    Deserialize, Deserializer, Serialize,
};
use serde_repr::{Deserialize_repr, Serialize_repr};
use serde_with::serde_as;
use sorella_db_databases::{
    clickhouse::{self, InsertRow, Row},
    fixed_string::FixedString,
};
use strum::EnumIter;

use crate::{
    tree::GasDetails, vec_b256, vec_fixed_string, vec_u256, vec_vec_fixed_string, vec_vec_u256,
};

#[serde_as]
#[derive(Debug, Deserialize, Row, Clone, Default)]
pub struct MevBlock {
    #[serde_as(as = "FixedString")]
    pub block_hash: B256,
    pub block_number: u64,
    pub mev_count: u64,
    pub finalized_eth_price: f64,
    /// Gas
    pub cumulative_gas_used: u128,
    pub cumulative_gas_paid: u128,
    pub total_bribe: u128,
    pub cumulative_mev_priority_fee_paid: u128,
    /// Builder address (recipient of coinbase.transfers)
    #[serde_as(as = "FixedString")]
    pub builder_address: Address,
    pub builder_eth_profit: i128,
    pub builder_finalized_profit_usd: f64,
    /// Proposer address
    #[serde(deserialize_with = "deser_option_address")]
    pub proposer_fee_recipient: Option<Address>,
    pub proposer_mev_reward: Option<u128>,
    pub proposer_finalized_profit_usd: Option<f64>,
    /// gas used * (effective gas price - base fee) for all Classified MEV txs
    /// Mev profit
    pub cumulative_mev_finalized_profit_usd: f64,
}

impl Serialize for MevBlock {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut ser_struct = serializer.serialize_struct("MevBlock", 15)?;

        let fixed_string = FixedString::new(format!("{:?}", self.block_hash));
        ser_struct.serialize_field("block_hash", &fixed_string)?;

        ser_struct.serialize_field("block_number", &self.block_number)?;
        ser_struct.serialize_field("mev_count", &self.mev_count)?;
        ser_struct.serialize_field("finalized_eth_price", &self.finalized_eth_price)?;
        ser_struct.serialize_field("cumulative_gas_used", &self.cumulative_gas_used)?;
        ser_struct.serialize_field("cumulative_gas_paid", &self.cumulative_gas_paid)?;
        ser_struct.serialize_field("total_bribe", &self.total_bribe)?;
        ser_struct.serialize_field("cumulative_mev_priority_fee_paid", &self.cumulative_mev_priority_fee_paid)?;

        let fixed_string = FixedString::new(format!("{:?}", self.builder_address));
        ser_struct.serialize_field("builder_address", &fixed_string)?;

        ser_struct.serialize_field("builder_eth_profit", &self.builder_eth_profit)?;
        ser_struct.serialize_field("builder_finalized_profit_usd", &self.builder_finalized_profit_usd)?;

        let fee_recep = if self.proposer_fee_recipient.is_none() {
            "".to_string()
        } else {
            format!("{:?}", self.proposer_fee_recipient.as_ref().unwrap())
        };
        let fixed_string = FixedString::new(fee_recep);
        ser_struct.serialize_field("proposer_fee_recipient", &fixed_string)?;

        ser_struct.serialize_field("proposer_mev_reward", &self.proposer_mev_reward)?;
        ser_struct.serialize_field("proposer_finalized_profit_usd", &self.proposer_finalized_profit_usd)?;
        ser_struct.serialize_field("cumulative_mev_finalized_profit_usd", &self.cumulative_mev_finalized_profit_usd)?;

        ser_struct.end()
    }
}

#[inline(always)]
fn deser_option_address<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Address>, D::Error> {
    println!("deser option addr");
    let s = FixedString::deserialize(deserializer)?;
    println!("deser fixed str {:?}", s);
    Ok(s.string.parse::<Address>().ok())
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Row, Clone, Default)]
pub struct ClassifiedMev {
    // can be multiple for sandwich
    pub block_number:         u64,
    #[serde_as(as = "FixedString")]
    pub tx_hash:              B256,
    #[serde_as(as = "FixedString")]
    pub eoa:                  Address,
    #[serde_as(as = "FixedString")]
    pub mev_contract:         Address,
    #[serde(with = "vec_fixed_string")]
    pub mev_profit_collector: Vec<Address>,
    pub finalized_profit_usd: f64,
    pub finalized_bribe_usd:  f64,
    pub mev_type:             MevType,
}

#[derive(
    Debug, Serialize_repr, Deserialize_repr, PartialEq, Eq, Hash, EnumIter, Clone, Copy, Default,
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

impl Row for MevType {
    const COLUMN_NAMES: &'static [&'static str] = &[];
}

pub trait SpecificMev:
    InsertRow + erased_serde::Serialize + Send + Sync + Debug + 'static + DynClone
{
    fn into_any(self: Box<Self>) -> Box<dyn Any + Send + Sync>;
    fn mev_type(&self) -> MevType;
    fn priority_fee_paid(&self) -> u128;
    fn bribe(&self) -> u128;
    fn mev_transaction_hashes(&self) -> Vec<B256>;
}

dyn_clone::clone_trait_object!(SpecificMev);

impl InsertRow for Box<dyn SpecificMev> {
    fn get_column_names(&self) -> &'static [&'static str] {
        (**self).get_column_names()
    }
}

impl serde::Serialize for Box<dyn SpecificMev> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut tup = serializer.serialize_tuple(2)?;
        let mev_type = self.mev_type();
        tup.serialize_element(&mev_type)?;
        let any = self.clone().into_any();

        match mev_type {
            MevType::Sandwich => {
                let this = any.downcast_ref::<Sandwich>().unwrap();
                tup.serialize_element(&this)?;
            }
            MevType::Backrun => {
                let this = any.downcast_ref::<AtomicBackrun>().unwrap();
                tup.serialize_element(&this)?;
            }
            MevType::JitSandwich => {
                let this = any.downcast_ref::<JitLiquiditySandwich>().unwrap();
                tup.serialize_element(&this)?;
            }
            MevType::Jit => {
                let this = any.downcast_ref::<JitLiquidity>().unwrap();
                tup.serialize_element(&this)?;
            }
            MevType::CexDex => {
                let this = any.downcast_ref::<CexDex>().unwrap();
                tup.serialize_element(&this)?;
            }
            MevType::Liquidation => {
                let this = any.downcast_ref::<Liquidation>().unwrap();
                tup.serialize_element(&this)?;
            }
            MevType::Unknown => unimplemented!("none yet"),
        }
        tup.end()
    }
}

macro_rules! decode_specific {
    ($mev_type:ident, $value:ident, $($mev:ident = $name:ident),+) => {
        match $mev_type {
        $(
            MevType::$mev => Box::new(serde_json::from_value::<$name>($value).unwrap()) as Box<dyn SpecificMev>,
        )+
        _ => todo!("missing variant")
    }
    };
}

impl<'de> serde::Deserialize<'de> for Box<dyn SpecificMev> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        println!("deseralizing");
        let (mev_type, val) = <(MevType, serde_json::Value)>::deserialize(deserializer)?;
        println!("{mev_type:?}, {val:#?}");

        Ok(decode_specific!(
            mev_type,
            val,
            Backrun = AtomicBackrun,
            Jit = JitLiquidity,
            JitSandwich = JitLiquiditySandwich,
            Sandwich = Sandwich,
            CexDex = CexDex,
            Liquidation = Liquidation
        ))
    }
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Row, Clone, Default)]
pub struct Sandwich {
    #[serde_as(as = "FixedString")]
    pub frontrun_tx_hash: B256,
    #[serde(with = "gas_details_tuple")]
    pub frontrun_gas_details: GasDetails,
    #[serde(rename = "frontrun_swaps.index")]
    pub frontrun_swaps_index: Vec<u64>,
    #[serde(with = "vec_fixed_string")]
    #[serde(rename = "frontrun_swaps.from")]
    pub frontrun_swaps_from: Vec<Address>,
    #[serde(with = "vec_fixed_string")]
    #[serde(rename = "frontrun_swaps.pool")]
    pub frontrun_swaps_pool: Vec<Address>,
    #[serde(with = "vec_fixed_string")]
    #[serde(rename = "frontrun_swaps.token_in")]
    pub frontrun_swaps_token_in: Vec<Address>,
    #[serde(with = "vec_fixed_string")]
    #[serde(rename = "frontrun_swaps.token_out")]
    pub frontrun_swaps_token_out: Vec<Address>,
    #[serde(with = "vec_u256")]
    #[serde(rename = "frontrun_swaps.amount_in")]
    pub frontrun_swaps_amount_in: Vec<U256>,
    #[serde(with = "vec_u256")]
    #[serde(rename = "frontrun_swaps.amount_out")]
    pub frontrun_swaps_amount_out: Vec<U256>,
    #[serde(with = "vec_b256")]
    pub victim_tx_hashes: Vec<B256>,
    #[serde(with = "vec_b256")]
    #[serde(rename = "victim_swaps.tx_hash")]
    pub victim_swaps_tx_hash: Vec<B256>,
    #[serde(rename = "victim_swaps.index")]
    pub victim_swaps_index: Vec<u64>,
    #[serde(with = "vec_fixed_string")]
    #[serde(rename = "victim_swaps.from")]
    pub victim_swaps_from: Vec<Address>,
    #[serde(with = "vec_fixed_string")]
    #[serde(rename = "victim_swaps.pool")]
    pub victim_swaps_pool: Vec<Address>,
    #[serde(with = "vec_fixed_string")]
    #[serde(rename = "victim_swaps.token_in")]
    pub victim_swaps_token_in: Vec<Address>,
    #[serde(with = "vec_fixed_string")]
    #[serde(rename = "victim_swaps.token_out")]
    pub victim_swaps_token_out: Vec<Address>,
    #[serde(with = "vec_u256")]
    #[serde(rename = "victim_swaps.amount_in")]
    pub victim_swaps_amount_in: Vec<U256>,
    #[serde(with = "vec_u256")]
    #[serde(rename = "victim_swaps.amount_out")]
    pub victim_swaps_amount_out: Vec<U256>,
    #[serde(rename = "victim_gas_details.coinbase_transfer")]
    pub victim_gas_details_coinbase_transfer: Vec<Option<u128>>,
    #[serde(rename = "victim_gas_details.priority_fee")]
    pub victim_gas_details_priority_fee: Vec<u128>,
    #[serde(rename = "victim_gas_details.gas_used")]
    pub victim_gas_details_gas_used: Vec<u128>,
    #[serde(rename = "victim_gas_details.effective_gas_price")]
    pub victim_gas_details_effective_gas_price: Vec<u128>,
    #[serde_as(as = "FixedString")]
    pub backrun_tx_hash: B256,
    #[serde(with = "gas_details_tuple")]
    pub backrun_gas_details: GasDetails,
    #[serde(rename = "backrun_swaps.index")]
    pub backrun_swaps_index: Vec<u64>,
    #[serde(with = "vec_fixed_string")]
    #[serde(rename = "backrun_swaps.from")]
    pub backrun_swaps_from: Vec<Address>,
    #[serde(with = "vec_fixed_string")]
    #[serde(rename = "backrun_swaps.pool")]
    pub backrun_swaps_pool: Vec<Address>,
    #[serde(with = "vec_fixed_string")]
    #[serde(rename = "backrun_swaps.token_in")]
    pub backrun_swaps_token_in: Vec<Address>,
    #[serde(with = "vec_fixed_string")]
    #[serde(rename = "backrun_swaps.token_out")]
    pub backrun_swaps_token_out: Vec<Address>,
    #[serde(with = "vec_u256")]
    #[serde(rename = "backrun_swaps.amount_in")]
    pub backrun_swaps_amount_in: Vec<U256>,
    #[serde(with = "vec_u256")]
    #[serde(rename = "backrun_swaps.amount_out")]
    pub backrun_swaps_amount_out: Vec<U256>,
}

pub fn compose_sandwich_jit(
    sandwich: Box<dyn Any>,
    jit: Box<dyn Any>,
    sandwich_classified: ClassifiedMev,
    jit_classified: ClassifiedMev,
) -> (ClassifiedMev, Box<dyn SpecificMev>) {
    let sandwich: Sandwich = *sandwich.downcast().unwrap();
    let jit: JitLiquidity = *jit.downcast().unwrap();

    let jit_sand = Box::new(JitLiquiditySandwich {
        frontrun_tx_hash: sandwich.frontrun_tx_hash,
        frontrun_gas_details: sandwich.frontrun_gas_details,
        frontrun_swaps_index: sandwich.frontrun_swaps_index,
        frontrun_swaps_from: sandwich.frontrun_swaps_from,
        frontrun_swaps_pool: sandwich.frontrun_swaps_pool,
        frontrun_swaps_token_in: sandwich.frontrun_swaps_token_in,
        frontrun_swaps_token_out: sandwich.frontrun_swaps_token_out,
        frontrun_swaps_amount_in: sandwich.frontrun_swaps_amount_in,
        frontrun_swaps_amount_out: sandwich.frontrun_swaps_amount_out,
        frontrun_mints_index: jit.jit_mints_index.into_iter().map(|x| x as u64).collect(),
        frontrun_mints_from: jit.jit_mints_from,
        frontrun_mints_to: jit.jit_mints_to,
        frontrun_mints_recipient: jit.jit_mints_recipient,
        frontrun_mints_tokens: jit.jit_mints_tokens,
        frontrun_mints_amounts: jit.jit_mints_amounts,
        victim_tx_hashes: sandwich.victim_tx_hashes,
        victim_swaps_tx_hash: sandwich.victim_swaps_tx_hash,
        victim_swaps_index: sandwich.victim_swaps_index,
        victim_swaps_from: sandwich.victim_swaps_from,
        victim_swaps_pool: sandwich.victim_swaps_pool,
        victim_swaps_token_in: sandwich.victim_swaps_token_in,
        victim_swaps_token_out: sandwich.victim_swaps_token_out,
        victim_swaps_amount_in: sandwich.victim_swaps_amount_in,
        victim_swaps_amount_out: sandwich.victim_swaps_amount_out,
        victim_gas_details_coinbase_transfer: sandwich.victim_gas_details_coinbase_transfer,
        victim_gas_details_priority_fee: sandwich.victim_gas_details_priority_fee,
        victim_gas_details_gas_used: sandwich.victim_gas_details_gas_used,
        victim_gas_details_effective_gas_price: sandwich.victim_gas_details_effective_gas_price,
        backrun_tx_hash: sandwich.backrun_tx_hash,
        backrun_gas_details: sandwich.backrun_gas_details,
        backrun_swaps_index: sandwich.backrun_swaps_index,
        backrun_swaps_from: sandwich.backrun_swaps_from,
        backrun_swaps_pool: sandwich.backrun_swaps_pool,
        backrun_swaps_token_in: sandwich.backrun_swaps_token_in,
        backrun_swaps_token_out: sandwich.backrun_swaps_token_out,
        backrun_swaps_amount_in: sandwich.backrun_swaps_amount_in,
        backrun_swaps_amount_out: sandwich.backrun_swaps_amount_out,
        backrun_burns_index: jit.jit_burns_index.into_iter().map(|x| x as u64).collect(),
        backrun_burns_from: jit.jit_burns_from,
        backrun_burns_to: jit.jit_burns_to,
        backrun_burns_recipient: jit.jit_burns_recipient,
        backrun_burns_tokens: jit.jit_burns_tokens,
        backrun_burns_amounts: jit.jit_burns_amounts,
    });

    let new_classifed = ClassifiedMev {
        tx_hash:              sandwich.frontrun_tx_hash,
        mev_type:             MevType::JitSandwich,
        block_number:         sandwich_classified.block_number,
        eoa:                  jit_classified.eoa,
        mev_contract:         sandwich_classified.mev_contract,
        mev_profit_collector: sandwich_classified.mev_profit_collector,
        finalized_bribe_usd:  sandwich_classified.finalized_bribe_usd,
        finalized_profit_usd: sandwich_classified.finalized_profit_usd
            + jit_classified.finalized_profit_usd,
    };

    (new_classifed, jit_sand)
}

impl SpecificMev for Sandwich {
    fn into_any(self: Box<Self>) -> Box<dyn Any + Send + Sync> {
        self
    }

    fn mev_type(&self) -> MevType {
        MevType::Sandwich
    }

    fn priority_fee_paid(&self) -> u128 {
        self.frontrun_gas_details.priority_fee + self.backrun_gas_details.priority_fee
    }

    fn bribe(&self) -> u128 {
        self.frontrun_gas_details.coinbase_transfer.unwrap_or(0)
            + self.backrun_gas_details.coinbase_transfer.unwrap_or(0)
    }

    fn mev_transaction_hashes(&self) -> Vec<B256> {
        let mut mev = vec![self.frontrun_tx_hash, self.backrun_tx_hash];
        // we add victim hashes in-case they register as a backrun
        mev.extend(self.victim_tx_hashes.clone());
        mev
    }
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Row, Clone, Default)]
pub struct JitLiquiditySandwich {
    #[serde_as(as = "FixedString")]
    pub frontrun_tx_hash: B256,
    #[serde(with = "gas_details_tuple")]
    pub frontrun_gas_details: GasDetails,
    #[serde(rename = "frontrun_swaps.index")]
    pub frontrun_swaps_index: Vec<u64>,
    #[serde(with = "vec_fixed_string")]
    #[serde(rename = "frontrun_swaps.from")]
    pub frontrun_swaps_from: Vec<Address>,
    #[serde(with = "vec_fixed_string")]
    #[serde(rename = "frontrun_swaps.pool")]
    pub frontrun_swaps_pool: Vec<Address>,
    #[serde(with = "vec_fixed_string")]
    #[serde(rename = "frontrun_swaps.token_in")]
    pub frontrun_swaps_token_in: Vec<Address>,
    #[serde(with = "vec_fixed_string")]
    #[serde(rename = "frontrun_swaps.token_out")]
    pub frontrun_swaps_token_out: Vec<Address>,
    #[serde(with = "vec_u256")]
    #[serde(rename = "frontrun_swaps.amount_in")]
    pub frontrun_swaps_amount_in: Vec<U256>,
    #[serde(with = "vec_u256")]
    #[serde(rename = "frontrun_swaps.amount_out")]
    pub frontrun_swaps_amount_out: Vec<U256>,
    #[serde(rename = "frontrun_mints.index")]
    pub frontrun_mints_index: Vec<u64>,
    #[serde(with = "vec_fixed_string")]
    #[serde(rename = "frontrun_mints.from")]
    pub frontrun_mints_from: Vec<Address>,
    #[serde(with = "vec_fixed_string")]
    #[serde(rename = "frontrun_mints.to")]
    pub frontrun_mints_to: Vec<Address>,
    #[serde(with = "vec_fixed_string")]
    #[serde(rename = "frontrun_mints.recipient")]
    pub frontrun_mints_recipient: Vec<Address>,
    #[serde(with = "vec_vec_fixed_string")]
    #[serde(rename = "frontrun_mints.tokens")]
    pub frontrun_mints_tokens: Vec<Vec<Address>>,
    #[serde(with = "vec_vec_u256")]
    #[serde(rename = "frontrun_mints.amounts")]
    pub frontrun_mints_amounts: Vec<Vec<U256>>,
    #[serde(with = "vec_b256")]
    pub victim_tx_hashes: Vec<B256>,
    #[serde(with = "vec_b256")]
    #[serde(rename = "victim_swaps.tx_hash")]
    pub victim_swaps_tx_hash: Vec<B256>,
    #[serde(rename = "victim_swaps.index")]
    pub victim_swaps_index: Vec<u64>,
    #[serde(with = "vec_fixed_string")]
    #[serde(rename = "victim_swaps.from")]
    pub victim_swaps_from: Vec<Address>,
    #[serde(with = "vec_fixed_string")]
    #[serde(rename = "victim_swaps.pool")]
    pub victim_swaps_pool: Vec<Address>,
    #[serde(with = "vec_fixed_string")]
    #[serde(rename = "victim_swaps.token_in")]
    pub victim_swaps_token_in: Vec<Address>,
    #[serde(with = "vec_fixed_string")]
    #[serde(rename = "victim_swaps.token_out")]
    pub victim_swaps_token_out: Vec<Address>,
    #[serde(with = "vec_u256")]
    #[serde(rename = "victim_swaps.amount_in")]
    pub victim_swaps_amount_in: Vec<U256>,
    #[serde(with = "vec_u256")]
    #[serde(rename = "victim_swaps.amount_out")]
    pub victim_swaps_amount_out: Vec<U256>,
    #[serde(rename = "victim_gas_details.coinbase_transfer")]
    pub victim_gas_details_coinbase_transfer: Vec<Option<u128>>,
    #[serde(rename = "victim_gas_details.priority_fee")]
    pub victim_gas_details_priority_fee: Vec<u128>,
    #[serde(rename = "victim_gas_details.gas_used")]
    pub victim_gas_details_gas_used: Vec<u128>,
    #[serde(rename = "victim_gas_details.effective_gas_price")]
    pub victim_gas_details_effective_gas_price: Vec<u128>,
    #[serde_as(as = "FixedString")]
    pub backrun_tx_hash: B256,
    #[serde(with = "gas_details_tuple")]
    pub backrun_gas_details: GasDetails,
    #[serde(rename = "backrun_swaps.index")]
    pub backrun_swaps_index: Vec<u64>,
    #[serde(with = "vec_fixed_string")]
    #[serde(rename = "backrun_swaps.from")]
    pub backrun_swaps_from: Vec<Address>,
    #[serde(with = "vec_fixed_string")]
    #[serde(rename = "backrun_swaps.pool")]
    pub backrun_swaps_pool: Vec<Address>,
    #[serde(with = "vec_fixed_string")]
    #[serde(rename = "backrun_swaps.token_in")]
    pub backrun_swaps_token_in: Vec<Address>,
    #[serde(with = "vec_fixed_string")]
    #[serde(rename = "backrun_swaps.token_out")]
    pub backrun_swaps_token_out: Vec<Address>,
    #[serde(with = "vec_u256")]
    #[serde(rename = "backrun_swaps.amount_in")]
    pub backrun_swaps_amount_in: Vec<U256>,
    #[serde(with = "vec_u256")]
    #[serde(rename = "backrun_swaps.amount_out")]
    pub backrun_swaps_amount_out: Vec<U256>,
    #[serde(rename = "backrun_burns.index")]
    pub backrun_burns_index: Vec<u64>,
    #[serde(with = "vec_fixed_string")]
    #[serde(rename = "backrun_burns.from")]
    pub backrun_burns_from: Vec<Address>,
    #[serde(with = "vec_fixed_string")]
    #[serde(rename = "backrun_burns.to")]
    pub backrun_burns_to: Vec<Address>,
    #[serde(with = "vec_fixed_string")]
    #[serde(rename = "backrun_burns.recipient")]
    pub backrun_burns_recipient: Vec<Address>,
    #[serde(with = "vec_vec_fixed_string")]
    #[serde(rename = "backrun_burns.tokens")]
    pub backrun_burns_tokens: Vec<Vec<Address>>,
    #[serde(with = "vec_vec_u256")]
    #[serde(rename = "backrun_burns.amounts")]
    pub backrun_burns_amounts: Vec<Vec<U256>>,
}

impl SpecificMev for JitLiquiditySandwich {
    fn into_any(self: Box<Self>) -> Box<dyn Any + Send + Sync> {
        self
    }

    fn mev_type(&self) -> MevType {
        MevType::JitSandwich
    }

    fn priority_fee_paid(&self) -> u128 {
        self.frontrun_gas_details.priority_fee + self.backrun_gas_details.priority_fee
    }

    fn bribe(&self) -> u128 {
        self.frontrun_gas_details.coinbase_transfer.unwrap_or(0)
            + self.backrun_gas_details.coinbase_transfer.unwrap_or(0)
    }

    fn mev_transaction_hashes(&self) -> Vec<B256> {
        vec![self.frontrun_tx_hash, self.backrun_tx_hash]
    }
}

#[derive(Debug, Serialize_repr, Deserialize_repr, PartialEq, Eq, Hash, EnumIter, Clone, Copy)]
#[repr(u8)]
#[allow(non_camel_case_types)]
#[serde(rename_all = "lowercase")]
pub enum PriceKind {
    Cex = 0,
    Dex = 1,
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Row, Clone, Default)]
pub struct CexDex {
    #[serde_as(as = "FixedString")]
    pub tx_hash:          B256,
    #[serde(rename = "swaps.index")]
    pub swaps_index:      Vec<u64>,
    #[serde(with = "vec_fixed_string")]
    #[serde(rename = "swaps.from")]
    pub swaps_from:       Vec<Address>,
    #[serde(with = "vec_fixed_string")]
    #[serde(rename = "swaps.pool")]
    pub swaps_pool:       Vec<Address>,
    #[serde(with = "vec_fixed_string")]
    #[serde(rename = "swaps.token_in")]
    pub swaps_token_in:   Vec<Address>,
    #[serde(with = "vec_fixed_string")]
    #[serde(rename = "swaps.token_out")]
    pub swaps_token_out:  Vec<Address>,
    #[serde(with = "vec_u256")]
    #[serde(rename = "swaps.amount_in")]
    pub swaps_amount_in:  Vec<U256>,
    #[serde(with = "vec_u256")]
    #[serde(rename = "swaps.amount_out")]
    pub swaps_amount_out: Vec<U256>,
    #[serde(with = "gas_details_tuple")]
    pub gas_details:      GasDetails,
    #[serde(rename = "prices.kind")]
    pub prices_kind:      Vec<PriceKind>,
    #[serde(with = "vec_fixed_string")]
    #[serde(rename = "prices.address")]
    pub prices_address:   Vec<Address>,
    #[serde(rename = "prices.price")]
    pub prices_price:     Vec<f64>,
}

impl SpecificMev for CexDex {
    fn into_any(self: Box<Self>) -> Box<dyn Any + Send + Sync> {
        self
    }

    fn mev_type(&self) -> MevType {
        MevType::CexDex
    }

    fn priority_fee_paid(&self) -> u128 {
        self.gas_details.priority_fee
    }

    fn mev_transaction_hashes(&self) -> Vec<B256> {
        vec![self.tx_hash]
    }

    fn bribe(&self) -> u128 {
        self.gas_details.coinbase_transfer.unwrap_or(0)
    }
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Row, Clone, Default)]
pub struct Liquidation {
    #[serde_as(as = "FixedString")]
    pub liquidation_tx_hash: B256,
    #[serde_as(as = "FixedString")]
    pub trigger: B256,
    #[serde(rename = "liquidation_swaps.index")]
    pub liquidation_swaps_index: Vec<u64>,
    #[serde(with = "vec_fixed_string")]
    #[serde(rename = "liquidation_swaps.from")]
    pub liquidation_swaps_from: Vec<Address>,
    #[serde(with = "vec_fixed_string")]
    #[serde(rename = "liquidation_swaps.pool")]
    pub liquidation_swaps_pool: Vec<Address>,
    #[serde(with = "vec_fixed_string")]
    #[serde(rename = "liquidation_swaps.token_in")]
    pub liquidation_swaps_token_in: Vec<Address>,
    #[serde(with = "vec_fixed_string")]
    #[serde(rename = "liquidation_swaps.token_out")]
    pub liquidation_swaps_token_out: Vec<Address>,
    #[serde(with = "vec_u256")]
    #[serde(rename = "liquidation_swaps.amount_in")]
    pub liquidation_swaps_amount_in: Vec<U256>,
    #[serde(with = "vec_u256")]
    #[serde(rename = "liquidation_swaps.amount_out")]
    pub liquidation_swaps_amount_out: Vec<U256>,
    #[serde(rename = "liquidations.index")]
    pub liquidations_index: Vec<u64>,
    #[serde(with = "vec_fixed_string")]
    #[serde(rename = "liquidations.liquidator")]
    pub liquidations_liquidator: Vec<Address>,
    #[serde(with = "vec_fixed_string")]
    #[serde(rename = "liquidations.liquidatee")]
    pub liquidations_liquidatee: Vec<Address>,
    #[serde(with = "vec_vec_fixed_string")]
    #[serde(rename = "liquidations.tokens")]
    pub liquidations_tokens: Vec<Vec<Address>>,
    #[serde(with = "vec_vec_u256")]
    #[serde(rename = "liquidations.amounts")]
    pub liquidations_amounts: Vec<Vec<U256>>,
    #[serde(rename = "liquidations.rewards")]
    pub liquidations_rewards: Vec<Vec<u128>>,
    #[serde(with = "gas_details_tuple")]
    pub gas_details: GasDetails,
}

impl SpecificMev for Liquidation {
    fn into_any(self: Box<Self>) -> Box<dyn Any + Send + Sync> {
        self
    }

    fn mev_type(&self) -> MevType {
        MevType::Liquidation
    }

    fn mev_transaction_hashes(&self) -> Vec<B256> {
        vec![self.liquidation_tx_hash]
    }

    fn priority_fee_paid(&self) -> u128 {
        self.gas_details.priority_fee
    }

    fn bribe(&self) -> u128 {
        self.gas_details.coinbase_transfer.unwrap_or(0)
    }
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Row, Clone, Default)]
pub struct JitLiquidity {
    #[serde_as(as = "FixedString")]
    pub mint_tx_hash: B256,
    #[serde(with = "gas_details_tuple")]
    pub mint_gas_details: GasDetails,
    #[serde(rename = "jit_mints.index")]
    pub jit_mints_index: Vec<u16>,
    #[serde(with = "vec_fixed_string")]
    #[serde(rename = "jit_mints.from")]
    pub jit_mints_from: Vec<Address>,
    #[serde(with = "vec_fixed_string")]
    #[serde(rename = "jit_mints.to")]
    pub jit_mints_to: Vec<Address>,
    #[serde(with = "vec_fixed_string")]
    #[serde(rename = "jit_mints.recipient")]
    pub jit_mints_recipient: Vec<Address>,
    #[serde(with = "vec_vec_fixed_string")]
    #[serde(rename = "jit_mints.tokens")]
    pub jit_mints_tokens: Vec<Vec<Address>>,
    #[serde(with = "vec_vec_u256")]
    #[serde(rename = "jit_mints.amounts")]
    pub jit_mints_amounts: Vec<Vec<U256>>,
    #[serde(with = "vec_b256")]
    pub victim_swap_tx_hashes: Vec<B256>,
    #[serde(with = "vec_b256")]
    #[serde(rename = "victim_swaps.tx_hash")]
    pub victim_swaps_tx_hash: Vec<B256>,
    #[serde(rename = "victim_swaps.index")]
    pub victim_swaps_index: Vec<u16>,
    #[serde(with = "vec_fixed_string")]
    #[serde(rename = "victim_swaps.from")]
    pub victim_swaps_from: Vec<Address>,
    #[serde(with = "vec_fixed_string")]
    #[serde(rename = "victim_swaps.pool")]
    pub victim_swaps_pool: Vec<Address>,
    #[serde(with = "vec_fixed_string")]
    #[serde(rename = "victim_swaps.token_in")]
    pub victim_swaps_token_in: Vec<Address>,
    #[serde(with = "vec_fixed_string")]
    #[serde(rename = "victim_swaps.token_out")]
    pub victim_swaps_token_out: Vec<Address>,
    #[serde(with = "vec_u256")]
    #[serde(rename = "victim_swaps.amount_in")]
    pub victim_swaps_amount_in: Vec<U256>,
    #[serde(with = "vec_u256")]
    #[serde(rename = "victim_swaps.amount_out")]
    pub victim_swaps_amount_out: Vec<U256>,
    #[serde(rename = "victim_gas_details.coinbase_transfer")]
    pub victim_gas_details_coinbase_transfer: Vec<Option<u128>>,
    #[serde(rename = "victim_gas_details.priority_fee")]
    pub victim_gas_details_priority_fee: Vec<u128>,
    #[serde(rename = "victim_gas_details.gas_used")]
    pub victim_gas_details_gas_used: Vec<u128>,
    #[serde(rename = "victim_gas_details.effective_gas_price")]
    pub victim_gas_details_effective_gas_price: Vec<u128>,
    #[serde_as(as = "FixedString")]
    pub burn_tx_hash: B256,
    #[serde(with = "gas_details_tuple")]
    pub burn_gas_details: GasDetails,
    #[serde(rename = "jit_burns.index")]
    pub jit_burns_index: Vec<u16>,
    #[serde(with = "vec_fixed_string")]
    #[serde(rename = "jit_burns.from")]
    pub jit_burns_from: Vec<Address>,
    #[serde(with = "vec_fixed_string")]
    #[serde(rename = "jit_burns.to")]
    pub jit_burns_to: Vec<Address>,
    #[serde(with = "vec_fixed_string")]
    #[serde(rename = "jit_burns.recipient")]
    pub jit_burns_recipient: Vec<Address>,
    #[serde(with = "vec_vec_fixed_string")]
    #[serde(rename = "jit_burns.tokens")]
    pub jit_burns_tokens: Vec<Vec<Address>>,
    #[serde(with = "vec_vec_u256")]
    #[serde(rename = "jit_burns.amounts")]
    pub jit_burns_amounts: Vec<Vec<U256>>,
}

impl SpecificMev for JitLiquidity {
    fn mev_type(&self) -> MevType {
        MevType::Jit
    }

    fn mev_transaction_hashes(&self) -> Vec<B256> {
        vec![self.mint_tx_hash, self.burn_tx_hash]
    }

    fn bribe(&self) -> u128 {
        self.mint_gas_details.coinbase_transfer.unwrap_or(0)
            + self.burn_gas_details.coinbase_transfer.unwrap_or(0)
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any + Send + Sync> {
        self
    }

    fn priority_fee_paid(&self) -> u128 {
        self.mint_gas_details.priority_fee + self.burn_gas_details.priority_fee
    }
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Row, Clone, Default)]
pub struct AtomicBackrun {
    #[serde_as(as = "FixedString")]
    pub tx_hash:          B256,
    #[serde(rename = "swaps.index")]
    pub swaps_index:      Vec<u64>,
    #[serde(with = "vec_fixed_string")]
    #[serde(rename = "swaps.from")]
    pub swaps_from:       Vec<Address>,
    #[serde(with = "vec_fixed_string")]
    #[serde(rename = "swaps.pool")]
    pub swaps_pool:       Vec<Address>,
    #[serde(with = "vec_fixed_string")]
    #[serde(rename = "swaps.token_in")]
    pub swaps_token_in:   Vec<Address>,
    #[serde(with = "vec_fixed_string")]
    #[serde(rename = "swaps.token_out")]
    pub swaps_token_out:  Vec<Address>,
    #[serde(rename = "swaps.amount_in")]
    #[serde(with = "vec_u256")]
    pub swaps_amount_in:  Vec<U256>,
    #[serde(rename = "swaps.amount_out")]
    #[serde(with = "vec_u256")]
    pub swaps_amount_out: Vec<U256>,
    #[serde(with = "gas_details_tuple")]
    pub gas_details:      GasDetails,
}

impl SpecificMev for AtomicBackrun {
    fn into_any(self: Box<Self>) -> Box<dyn Any + Send + Sync> {
        self
    }

    fn priority_fee_paid(&self) -> u128 {
        self.gas_details.priority_fee
    }

    fn bribe(&self) -> u128 {
        self.gas_details.coinbase_transfer.unwrap_or(0)
    }

    fn mev_transaction_hashes(&self) -> Vec<B256> {
        vec![self.tx_hash]
    }

    fn mev_type(&self) -> MevType {
        MevType::Backrun
    }
}

mod gas_details_tuple {
    use serde::{self, Deserialize, Deserializer, Serialize, Serializer};

    use super::GasDetails;

    pub fn serialize<S>(value: &GasDetails, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let tuple = (
            value.coinbase_transfer,
            value.priority_fee,
            value.gas_used,
            value.effective_gas_price,
        );
        tuple.serialize(serializer)
    }

    #[allow(dead_code)]
    pub fn deserialize<'de, D>(deserializer: D) -> Result<GasDetails, D::Error>
    where
        D: Deserializer<'de>,
    {
        let tuple = <(Option<u128>, u128, u128, u128)>::deserialize(deserializer)?;
        Ok(GasDetails {
            coinbase_transfer:   tuple.0.map(Into::into),
            priority_fee:        tuple.1,
            gas_used:            tuple.2,
            effective_gas_price: tuple.3,
        })
    }
}

#[cfg(test)]
mod tests {

    use std::{any::Any, str::FromStr};

    use sorella_db_databases::*;

    use super::*;
    use crate::test_utils::spawn_db;

    async fn insert_classified_data2_test<T: SpecificMev + serde::Serialize + Row + Clone>(
        db_client: &ClickhouseClient,
        mev_detail: Box<dyn Any>,
        table: &str,
    ) {
        let this = (*mev_detail).downcast_ref::<T>().unwrap();
        db_client.insert_one(this.clone(), table).await.unwrap();
    }

    async fn insert_classified_data_test(
        db_client: &ClickhouseClient,
        mev_details: Vec<(Box<dyn SpecificMev>, MevType)>,
        table: &str,
    ) {
        for (mev, mev_type) in mev_details {
            let mev = mev.into_any();
            match mev_type {
                MevType::Sandwich => {
                    insert_classified_data2_test::<Sandwich>(db_client, mev, SANDWICH_TABLE).await
                }
                MevType::Backrun => {
                    insert_classified_data2_test::<AtomicBackrun>(db_client, mev, BACKRUN_TABLE)
                        .await
                }
                MevType::JitSandwich => {
                    insert_classified_data2_test::<JitLiquiditySandwich>(
                        db_client,
                        mev,
                        JIT_SANDWICH_TABLE,
                    )
                    .await
                }
                MevType::Jit => {
                    insert_classified_data2_test::<JitLiquidity>(db_client, mev, JIT_TABLE).await
                }
                MevType::CexDex => {
                    insert_classified_data2_test::<CexDex>(db_client, mev, CEX_DEX_TABLE).await
                }
                MevType::Liquidation => {
                    insert_classified_data2_test::<Liquidation>(db_client, mev, LIQUIDATIONS_TABLE)
                        .await
                }
                MevType::Unknown => unimplemented!("none yet"),
            }
        }

        //let this = (*mev_detail).downcast_ref::<T>().unwrap();
        //db_client.insert_one(this.clone(), table).await.unwrap();
    }

    #[tokio::test]
    async fn test_db_mev_block() {
        let test_block = MevBlock::default();

        let db: ClickhouseClient = spawn_db();

        db.insert_one(test_block.clone(), MEV_BLOCKS_TABLE)
            .await
            .unwrap();

        db.execute(
            &format!(
                "DELETE FROM {MEV_BLOCKS_TABLE} where block_hash = '{:?}' and block_number = {}",
                test_block.block_hash, test_block.block_number
            ),
            &(),
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_db_classified_mev() {
        let test_mev = ClassifiedMev::default();

        let db = spawn_db();

        db.insert_one(test_mev.clone(), CLASSIFIED_MEV_TABLE)
            .await
            .unwrap();

        db.execute(
            &format!(
                "DELETE FROM {CLASSIFIED_MEV_TABLE} where tx_hash = '{:?}' and block_number = {}",
                test_mev.tx_hash, test_mev.block_number
            ),
            &(),
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_db_sandwhich() {
        let test_mev = Sandwich::default();

        let db = spawn_db();

        db.insert_one(test_mev.clone(), SANDWICH_TABLE)
            .await
            .unwrap();

        db.execute(
            &format!(
                "DELETE FROM {SANDWICH_TABLE} where frontrun_tx_hash = '{:?}' and backrun_tx_hash \
                 = 
         '{:?}'",
                test_mev.frontrun_tx_hash, test_mev.backrun_tx_hash
            ),
            &(),
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_db_jit_sandwhich() {
        let test_mev = JitLiquiditySandwich::default();

        let db = spawn_db();

        db.insert_one(test_mev.clone(), JIT_SANDWICH_TABLE)
            .await
            .unwrap();

        db.execute(
            &format!(
                "DELETE FROM {JIT_SANDWICH_TABLE} where frontrun_tx_hash = '{:?}' and \
                 backrun_tx_hash = 
         '{:?}'",
                test_mev.frontrun_tx_hash, test_mev.backrun_tx_hash
            ),
            &(),
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_db_jit() {
        let mut test_mev: JitLiquidity = JitLiquidity::default();
        test_mev.jit_mints_index.push(Default::default());
        test_mev.jit_mints_to.push(Default::default());
        test_mev.jit_mints_recipient.push(Default::default());
        test_mev.jit_mints_from.push(Default::default());
        test_mev.jit_mints_tokens.push(vec![Default::default()]);
        test_mev.jit_mints_amounts.push(vec![Default::default()]);
        test_mev.burn_gas_details.coinbase_transfer = None;
        test_mev.jit_burns_tokens = vec![vec![
            Address::from_str("0xb17548c7b510427baac4e267bea62e800b247173").unwrap(),
            Address::from_str("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2").unwrap(),
        ]];
        test_mev.jit_burns_index.push(Default::default());
        test_mev.jit_burns_to.push(Default::default());
        test_mev.jit_burns_recipient.push(Default::default());
        test_mev.jit_burns_from.push(Default::default());
        test_mev.jit_burns_amounts.push(vec![Default::default()]);

        let db = spawn_db();

        //db.insert_one(test_mev.clone(), JIT_TABLE).await.unwrap();
        let t: Box<dyn SpecificMev> = Box::new(test_mev.clone());

        insert_classified_data_test(&db, vec![(t, MevType::Jit)], JIT_TABLE).await;

        db.execute(
            &format!(
                "DELETE FROM {JIT_TABLE} where mint_tx_hash = '{:?}' and burn_tx_hash = '{:?}'",
                test_mev.mint_tx_hash, test_mev.burn_tx_hash
            ),
            &(),
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_db_liquidation() {
        let test_mev = Liquidation::default();

        let db = spawn_db();

        db.insert_one(test_mev.clone(), LIQUIDATIONS_TABLE)
            .await
            .unwrap();

        db.execute(
            &format!(
                "DELETE FROM {LIQUIDATIONS_TABLE} where liquidation_tx_hash = '{:?}' and trigger \
                 = '{:?}'",
                test_mev.liquidation_tx_hash, test_mev.trigger
            ),
            &(),
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_db_atomic_backrun() {
        let test_mev = AtomicBackrun::default();

        let db = spawn_db();

        db.insert_one(test_mev.clone(), BACKRUN_TABLE)
            .await
            .unwrap();

        db.execute(
            &format!("DELETE FROM {BACKRUN_TABLE} where tx_hash = '{:?}'", test_mev.tx_hash),
            &(),
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_db_cex_dex() {
        let test_mev = CexDex::default();

        let db = spawn_db();

        db.insert_one(test_mev.clone(), CEX_DEX_TABLE)
            .await
            .unwrap();

        db.execute(
            &format!("DELETE FROM {CEX_DEX_TABLE} where tx_hash = '{:?}'", test_mev.tx_hash),
            &(),
        )
        .await
        .unwrap();
    }
}
