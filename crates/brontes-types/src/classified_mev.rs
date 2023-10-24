use std::{any::Any, fmt::Debug};

use reth_primitives::{Address, H256};
use serde::{self, Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use serde_with::serde_as;
use sorella_db_databases::{
    clickhouse::{self, InsertRow, Row},
    fixed_string::FixedString,
};
use strum::EnumIter;

use super::normalized_actions::Actions;
use crate::tree::GasDetails;

#[serde_as]
#[derive(Debug, Serialize, Row, Clone)]
pub struct MevBlock {
    #[serde_as(as = "FixedString")]
    pub block_hash: H256,
    pub block_number: u64,
    pub mev_count: u64,
    pub submission_eth_price: f64,
    pub finalized_eth_price: f64,
    /// Gas
    pub cumulative_gas_used: u64,
    pub cumulative_gas_paid: u64,
    pub total_bribe: u64,
    pub cumulative_mev_priority_fee_paid: u64,
    /// Builder address (recipient of coinbase.transfers)
    #[serde_as(as = "FixedString")]
    pub builder_address: Address,
    pub builder_eth_profit: u64,
    pub builder_submission_profit_usd: f64,
    pub builder_finalized_profit_usd: f64,
    /// Proposer address
    #[serde_as(as = "FixedString")]
    pub proposer_fee_recipient: Address,
    pub proposer_mev_reward: u64,
    pub proposer_submission_profit_usd: f64,
    pub proposer_finalized_profit_usd: f64,
    // gas used * (effective gas price - base fee) for all Classified MEV txs
    /// Mev profit
    pub cumulative_mev_submission_profit_usd: f64,
    pub cumulative_mev_finalized_profit_usd: f64,
}

#[serde_as]
#[derive(Debug, Serialize, Row, Clone)]
pub struct ClassifiedMev {
    // can be multiple for sandwich
    pub block_number:          u64,
    #[serde_as(as = "FixedString")]
    pub tx_hash:               H256,
    #[serde_as(as = "FixedString")]
    pub eoa:                   Address,
    #[serde_as(as = "FixedString")]
    pub mev_contract:          Address,
    #[serde_as(as = "FixedString")]
    pub mev_profit_collector:  Address,
    pub mev_type:              MevType,
    pub submission_profit_usd: f64,
    pub finalized_profit_usd:  f64,
    pub submission_bribe_usd:  f64,
    pub finalized_bribe_usd:   f64,
}

#[derive(Debug, Serialize_repr, PartialEq, Eq, Hash, EnumIter, Clone, Copy)]
#[repr(u8)]
#[allow(non_camel_case_types)]
pub enum MevType {
    sandwich     = 1,
    backrun      = 5,
    jit_sandwich = 3,
    jit          = 2,
    cex_dex      = 0,
    liquidation  = 4,
    unknown      = 6,
}

impl Row for MevType {
    const COLUMN_NAMES: &'static [&'static str] = &[];
}

/// Because of annoying trait requirements. we do some degenerate shit here.
pub trait SpecificMev: InsertRow + erased_serde::Serialize + Send + Sync + Debug + 'static {
    fn into_any(self: Box<Self>) -> Box<dyn Any>;
    fn mev_type(&self) -> MevType;
    fn priority_fee_paid(&self) -> u64;
    fn bribe(&self) -> u64;
    fn mev_transaction_hashes(&self) -> Vec<H256>;
}

impl InsertRow for Box<dyn SpecificMev> {
    fn get_column_names(&self) -> &'static [&'static str] {
        (**self).get_column_names()
    }
}

impl serde::Serialize for dyn SpecificMev {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        erased_serde::serialize(self, serializer)
    }
}

#[derive(Debug, Serialize_repr, PartialEq, Eq, Hash, EnumIter, Clone, Copy)]
#[repr(u8)]
#[allow(non_camel_case_types)]
pub enum SwapKind {
    frontrun = 0,
    backrun  = 1,
    victim   = 2,
}

impl From<JitKind> for SwapKind {
    fn from(value: JitKind) -> Self {
        match value {
            JitKind::mint => SwapKind::frontrun,
            JitKind::burn => SwapKind::backrun,
            JitKind::swap => SwapKind::frontrun,
        }
    }
}

#[serde_as]
#[derive(Debug, Serialize, Row, Clone)]
pub struct Sandwich {
    #[serde_as(as = "FixedString")]
    pub frontrun_tx_hash: H256,
    #[serde_as(as = "FixedString")]
    pub backrun_tx_hash: H256,
    #[serde_as(as = "Vec<FixedString>")]
    pub victim_tx_hashes: Vec<H256>,
    #[serde(rename = "swaps.kinds")]
    pub swaps_kinds: Vec<SwapKind>,
    #[serde(rename = "swaps.tx_num")]
    pub swaps_tx_num: Vec<u8>,
    #[serde(rename = "swaps.index")]
    pub swaps_index: Vec<u16>,
    #[serde(rename = "swaps.from")]
    pub swaps_from: Vec<Address>,
    #[serde(rename = "swaps.pool")]
    pub swaps_pool: Vec<Address>,
    #[serde_as(as = "Vec<FixedString>")]
    #[serde(rename = "swaps.token_in")]
    pub swaps_token_in: Vec<Address>,
    #[serde_as(as = "Vec<FixedString>")]
    #[serde(rename = "swaps.token_out")]
    pub swaps_token_out: Vec<Address>,
    #[serde(rename = "swaps.amount_in")]
    pub swaps_amount_in: Vec<u64>,
    #[serde(rename = "swaps.amount_out")]
    pub swaps_amount_out: Vec<u64>,
    #[serde(rename = "gas_details.kind")]
    pub gas_details_kind: Vec<SwapKind>,
    #[serde(rename = "gas_details.tx_num")]
    pub gas_details_tx_num: Vec<u8>,
    #[serde(rename = "gas_details.coinbase_transfer")]
    pub gas_details_coinbase_transfer: Vec<Option<u64>>,
    #[serde(rename = "gas_details.priority_fee")]
    pub gas_details_priority_fee: Vec<u64>,
    #[serde(rename = "gas_details.gas_used")]
    pub gas_details_gas_used: Vec<u64>,
    #[serde(rename = "gas_details.effective_gas_price")]
    pub gas_details_effective_gas_price: Vec<u64>,
}

pub fn compose_sandwich_jit(
    sandwich: Box<dyn Any>,
    jit: Box<dyn Any>,
    sandwich_classified: ClassifiedMev,
    jit_classified: ClassifiedMev,
) -> (ClassifiedMev, Box<dyn SpecificMev>) {
    let sandwich: Sandwich = *sandwich.downcast().unwrap();
    let jit: JitLiquidity = *jit.downcast().unwrap();

    let mut gas_details_kinds = sandwich.gas_details_kind;
    jit.gas_details_kind
        .iter()
        .for_each(|t| gas_details_kinds.push((*t).into()));

    let mut gas_details_tx_num = sandwich.gas_details_tx_num;
    let mut i = 2;
    while i > 0 {
        gas_details_tx_num.push(gas_details_tx_num.last().unwrap() + 1);
        i -= 1;
    }

    let mut gas_details_coinbase_transfer = sandwich.gas_details_coinbase_transfer;
    jit.gas_details_coinbase_transfer
        .iter()
        .for_each(|t| gas_details_coinbase_transfer.push(*t));

    let mut gas_details_priority_fee = sandwich.gas_details_priority_fee;
    jit.gas_details_priority_fee
        .iter()
        .for_each(|t| gas_details_priority_fee.push(*t));

    let mut gas_details_gas_used = sandwich.gas_details_gas_used;
    jit.gas_details_gas_used
        .iter()
        .for_each(|t| gas_details_gas_used.push(*t));

    let mut gas_details_effective_gas_price = sandwich.gas_details_effective_gas_price;
    jit.gas_details_effective_gas_price
        .iter()
        .for_each(|t| gas_details_effective_gas_price.push(*t));

    let jit_sand = Box::new(JitLiquiditySandwich {
        frontrun_tx_hash: sandwich.frontrun_tx_hash,
        swap_tx_hash: jit.swap_tx_hash,
        burn_tx_hash: jit.burn_tx_hash,
        backrun_tx_hash: sandwich.backrun_tx_hash,
        victim_tx_hashes: sandwich.victim_tx_hashes,
        mints_burns_kind: jit.mints_burns_kind,
        mints_burns_index: jit.mints_burns_index,
        mints_burns_from: jit.mints_burns_from,
        mints_burns_to: jit.mints_burns_to,
        mints_burns_recipient: jit.mints_burns_recipient,
        mints_burns_tokens: jit.mints_burns_tokens,
        mints_burns_amounts: jit.mints_burns_amounts,
        swaps_kind: sandwich.swaps_kinds,
        swaps_tx_num: sandwich.swaps_tx_num,
        swaps_index: sandwich.swaps_index,
        swaps_from: sandwich.swaps_from,
        swaps_pool: sandwich.swaps_pool,
        swaps_token_in: sandwich.swaps_token_in,
        swaps_token_out: sandwich.swaps_token_out,
        swaps_amount_in: sandwich.swaps_amount_in,
        swaps_amount_out: sandwich.swaps_amount_out,
        gas_details_kind: gas_details_kinds,
        gas_details_tx_num,
        gas_details_coinbase_transfer,
        gas_details_priority_fee,
        gas_details_gas_used,
        gas_details_effective_gas_price,
    });

    let new_classifed = ClassifiedMev {
        tx_hash:               sandwich.frontrun_tx_hash,
        mev_type:              MevType::jit_sandwich,
        block_number:          sandwich_classified.block_number,
        eoa:                   jit_classified.eoa,
        mev_contract:          sandwich_classified.mev_contract,
        mev_profit_collector:  sandwich_classified.mev_profit_collector,
        finalized_bribe_usd:   sandwich_classified.finalized_bribe_usd,
        submission_bribe_usd:  sandwich_classified.submission_bribe_usd,
        submission_profit_usd: sandwich_classified.submission_profit_usd
            + jit_classified.submission_profit_usd,
        finalized_profit_usd:  sandwich_classified.finalized_profit_usd
            + jit_classified.finalized_profit_usd,
    };

    (new_classifed, jit_sand)
}

impl SpecificMev for Sandwich {
    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }

    fn mev_type(&self) -> MevType {
        MevType::sandwich
    }

    fn priority_fee_paid(&self) -> u64 {
        let gas_details = self
            .gas_details_kind
            .iter()
            .zip(self.gas_details_priority_fee.clone())
            .collect::<Vec<_>>();

        let mut priority_fee = 0;

        gas_details
            .into_iter()
            .for_each(|(det_type, det_amt)| match det_type {
                SwapKind::frontrun => priority_fee += det_amt,
                SwapKind::backrun => priority_fee += det_amt,
                _ => (),
            });

        priority_fee
    }

    fn bribe(&self) -> u64 {
        let gas_details = self
            .gas_details_kind
            .iter()
            .zip(self.gas_details_coinbase_transfer.clone())
            .collect::<Vec<_>>();

        let mut bribe = 0;

        gas_details
            .into_iter()
            .for_each(|(det_type, det_amt)| match det_type {
                SwapKind::frontrun => bribe += det_amt.unwrap_or(0),
                SwapKind::backrun => bribe += det_amt.unwrap_or(0),
                _ => (),
            });

        bribe
    }

    fn mev_transaction_hashes(&self) -> Vec<H256> {
        vec![self.frontrun_tx_hash, self.backrun_tx_hash]
    }
}

#[derive(Debug, Serialize_repr, PartialEq, Eq, Hash, EnumIter, Clone, Copy)]
#[repr(u8)]
#[allow(non_camel_case_types)]
pub enum MintBurnKind {
    frontrun = 0,
    backrun  = 1,
}

#[serde_as]
#[derive(Debug, Serialize, Row, Clone, Default)]
pub struct JitLiquiditySandwich {
    #[serde_as(as = "FixedString")]
    pub frontrun_tx_hash: H256,
    #[serde_as(as = "FixedString")]
    pub swap_tx_hash: H256,
    #[serde_as(as = "FixedString")]
    pub burn_tx_hash: H256,
    #[serde_as(as = "FixedString")]
    pub backrun_tx_hash: H256,
    #[serde_as(as = "Vec<FixedString>")]
    pub victim_tx_hashes: Vec<H256>,
    #[serde(rename = "mints_burns.kind")]
    pub mints_burns_kind: Vec<MintBurnKind>,
    #[serde(rename = "mints_burns.index")]
    pub mints_burns_index: Vec<u16>,
    #[serde_as(as = "Vec<FixedString>")]
    #[serde(rename = "mints_burns.from")]
    pub mints_burns_from: Vec<Address>,
    #[serde_as(as = "Vec<FixedString>")]
    #[serde(rename = "mints_burns.to")]
    pub mints_burns_to: Vec<Address>,
    #[serde_as(as = "Vec<FixedString>")]
    #[serde(rename = "mints_burns.recipient")]
    pub mints_burns_recipient: Vec<Address>,
    #[serde_as(as = "Vec<Vec<FixedString>>")]
    #[serde(rename = "mints_burns.tokens")]
    pub mints_burns_tokens: Vec<Vec<Address>>,
    #[serde(rename = "mints_burns.amounts")]
    pub mints_burns_amounts: Vec<Vec<u64>>,
    #[serde(rename = "swaps.kind")]
    pub swaps_kind: Vec<SwapKind>,
    #[serde(rename = "swaps.tx_num")]
    pub swaps_tx_num: Vec<u8>,
    #[serde(rename = "swaps.index")]
    pub swaps_index: Vec<u16>,
    #[serde_as(as = "Vec<FixedString>")]
    #[serde(rename = "swaps.from")]
    pub swaps_from: Vec<Address>,
    #[serde_as(as = "Vec<FixedString>")]
    #[serde(rename = "swaps.pool")]
    pub swaps_pool: Vec<Address>,
    #[serde_as(as = "Vec<FixedString>")]
    #[serde(rename = "swaps.token_in")]
    pub swaps_token_in: Vec<Address>,
    #[serde_as(as = "Vec<FixedString>")]
    #[serde(rename = "swaps.token_out")]
    pub swaps_token_out: Vec<Address>,
    #[serde(rename = "swaps.amount_in")]
    pub swaps_amount_in: Vec<u64>,
    #[serde(rename = "swaps.amount_out")]
    pub swaps_amount_out: Vec<u64>,
    #[serde(rename = "gas_details.kind")]
    pub gas_details_kind: Vec<SwapKind>,
    #[serde(rename = "gas_details.tx_num")]
    pub gas_details_tx_num: Vec<u8>,
    #[serde(rename = "gas_details.coinbase_transfer")]
    pub gas_details_coinbase_transfer: Vec<Option<u64>>,
    #[serde(rename = "gas_details.priority_fee")]
    pub gas_details_priority_fee: Vec<u64>,
    #[serde(rename = "gas_details.gas_used")]
    pub gas_details_gas_used: Vec<u64>,
    #[serde(rename = "gas_details.effective_gas_price")]
    pub gas_details_effective_gas_price: Vec<u64>,
}

impl SpecificMev for JitLiquiditySandwich {
    fn mev_type(&self) -> MevType {
        MevType::jit_sandwich
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }

    fn priority_fee_paid(&self) -> u64 {
        let gas_details = self
            .gas_details_kind
            .iter()
            .zip(self.gas_details_priority_fee.clone())
            .collect::<Vec<_>>();

        let mut priority_fee = 0;

        gas_details
            .into_iter()
            .for_each(|(det_type, det_amt)| match det_type {
                SwapKind::frontrun => priority_fee += det_amt,
                SwapKind::backrun => priority_fee += det_amt,
                _ => (),
            });

        priority_fee
    }

    fn bribe(&self) -> u64 {
        let gas_details = self
            .gas_details_kind
            .iter()
            .zip(self.gas_details_coinbase_transfer.clone())
            .collect::<Vec<_>>();

        let mut bribe = 0;

        gas_details
            .into_iter()
            .for_each(|(det_type, det_amt)| match det_type {
                SwapKind::frontrun => bribe += det_amt.unwrap_or(0),
                SwapKind::backrun => bribe += det_amt.unwrap_or(0),
                _ => (),
            });

        bribe
    }

    fn mev_transaction_hashes(&self) -> Vec<H256> {
        vec![self.frontrun_tx_hash, self.backrun_tx_hash]
    }
}

#[derive(Debug, Serialize_repr, PartialEq, Eq, Hash, EnumIter, Clone, Copy)]
#[repr(u8)]
#[allow(non_camel_case_types)]
pub enum PriceKind {
    cex = 0,
    dex = 1,
}

#[serde_as]
#[derive(Debug, Serialize, Row, Clone, Default)]
pub struct CexDex {
    #[serde_as(as = "FixedString")]
    pub tx_hash:          H256,
    #[serde(rename = "swaps.index")]
    pub swaps_index:      Vec<u16>,
    #[serde_as(as = "Vec<FixedString>")]
    #[serde(rename = "swaps.from")]
    pub swaps_from:       Vec<Address>,
    #[serde_as(as = "Vec<FixedString>")]
    #[serde(rename = "swaps.pool")]
    pub swaps_pool:       Vec<Address>,
    #[serde_as(as = "Vec<FixedString>")]
    #[serde(rename = "swaps.token_in")]
    pub swaps_token_in:   Vec<Address>,
    #[serde_as(as = "Vec<FixedString>")]
    #[serde(rename = "swaps.token_out")]
    pub swaps_token_out:  Vec<Address>,
    #[serde(rename = "swaps.amount_in")]
    pub swaps_amount_in:  Vec<u64>,
    #[serde(rename = "swaps.amount_out")]
    pub swaps_amount_out: Vec<u64>,
    #[serde(with = "gas_details_tuple")]
    pub gas_details:      GasDetails,
    #[serde(rename = "prices.kind")]
    pub prices_kind:      Vec<PriceKind>,
    #[serde(rename = "prices.symbol")]
    pub prices_symbol:    Vec<String>,
    #[serde(rename = "prices.price")]
    pub prices_price:     Vec<f64>,
}

impl SpecificMev for CexDex {
    fn mev_type(&self) -> MevType {
        MevType::cex_dex
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }

    fn priority_fee_paid(&self) -> u64 {
        self.gas_details.priority_fee
    }

    fn mev_transaction_hashes(&self) -> Vec<H256> {
        vec![self.tx_hash]
    }

    fn bribe(&self) -> u64 {
        self.gas_details.coinbase_transfer.unwrap_or(0) as u64
    }
}

#[serde_as]
#[derive(Debug, Serialize, Row, Clone, Default)]
pub struct Liquidation {
    #[serde_as(as = "FixedString")]
    pub liquidation_tx_hash: H256,
    #[serde_as(as = "FixedString")]
    pub trigger: H256,
    #[serde(rename = "liquidation_swaps.index")]
    pub liquidation_swaps_index: Vec<u16>,
    #[serde_as(as = "Vec<FixedString>")]
    #[serde(rename = "liquidation_swaps.from")]
    pub liquidation_swaps_from: Vec<Address>,
    #[serde_as(as = "Vec<FixedString>")]
    #[serde(rename = "liquidation_swaps.pool")]
    pub liquidation_swaps_pool: Vec<Address>,
    #[serde_as(as = "Vec<FixedString>")]
    #[serde(rename = "liquidation_swaps.token_in")]
    pub liquidation_swaps_token_in: Vec<Address>,
    #[serde_as(as = "Vec<FixedString>")]
    #[serde(rename = "liquidation_swaps.token_out")]
    pub liquidation_swaps_token_out: Vec<Address>,
    #[serde(rename = "liquidation_swaps.amount_in")]
    pub liquidation_swaps_amount_in: Vec<u128>,
    #[serde(rename = "liquidation_swaps.amount_out")]
    pub liquidation_swaps_amount_out: Vec<u128>,
    #[serde(rename = "liquidations.index")]
    pub liquidations_index: Vec<u16>,
    #[serde_as(as = "Vec<FixedString>")]
    #[serde(rename = "liquidations.liquidator")]
    pub liquidations_liquidator: Vec<Address>,
    #[serde_as(as = "Vec<FixedString>")]
    #[serde(rename = "liquidations.liquidatee")]
    pub liquidations_liquidatee: Vec<Address>,
    #[serde_as(as = "Vec<Vec<FixedString>>")]
    #[serde(rename = "liquidations.tokens")]
    pub liquidations_tokens: Vec<Vec<Address>>,
    #[serde(rename = "liquidations.amounts")]
    pub liquidations_amounts: Vec<Vec<u128>>,
    #[serde(rename = "liquidations.rewards")]
    pub liquidations_rewards: Vec<Vec<u128>>,
    #[serde(with = "gas_details_tuple")]
    pub gas_details: GasDetails,
}

impl SpecificMev for Liquidation {
    fn mev_type(&self) -> MevType {
        MevType::liquidation
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }

    fn mev_transaction_hashes(&self) -> Vec<H256> {
        vec![self.liquidation_tx_hash]
    }

    fn priority_fee_paid(&self) -> u64 {
        self.gas_details.priority_fee
    }

    fn bribe(&self) -> u64 {
        self.gas_details.coinbase_transfer.unwrap_or(0) as u64
    }
}

#[derive(Debug, Serialize_repr, PartialEq, Eq, Hash, EnumIter, Clone, Copy)]
#[repr(u8)]
#[allow(non_camel_case_types)]
pub enum JitKind {
    mint = 0,
    burn = 1,
    swap = 2,
}

#[serde_as]
#[derive(Debug, Serialize, Row, Clone, Default)]
pub struct JitLiquidity {
    #[serde_as(as = "FixedString")]
    pub mint_tx_hash: H256,
    #[serde_as(as = "FixedString")]
    pub swap_tx_hash: H256,
    #[serde_as(as = "FixedString")]
    pub burn_tx_hash: H256,
    #[serde(rename = "swaps.index")]
    pub swaps_index: Vec<u16>,
    #[serde_as(as = "Vec<FixedString>")]
    #[serde(rename = "swaps.from")]
    pub swaps_from: Vec<Address>,
    #[serde_as(as = "Vec<FixedString>")]
    #[serde(rename = "swaps.pool")]
    pub swaps_pool: Vec<Address>,
    #[serde_as(as = "Vec<FixedString>")]
    #[serde(rename = "swaps.token_in")]
    pub swaps_token_in: Vec<Address>,
    #[serde_as(as = "Vec<FixedString>")]
    #[serde(rename = "swaps.token_out")]
    pub swaps_token_out: Vec<Address>,
    #[serde(rename = "swaps.amount_in")]
    pub swaps_amount_in: Vec<u64>,
    #[serde(rename = "swaps.amount_out")]
    pub swaps_amount_out: Vec<u64>,
    #[serde(rename = "mints_burns.kind")]
    pub mints_burns_kind: Vec<MintBurnKind>,
    #[serde(rename = "mints_burns.index")]
    pub mints_burns_index: Vec<u16>,
    #[serde_as(as = "Vec<FixedString>")]
    #[serde(rename = "mints_burns.from")]
    pub mints_burns_from: Vec<Address>,
    #[serde_as(as = "Vec<FixedString>")]
    #[serde(rename = "mints_burns.to")]
    pub mints_burns_to: Vec<Address>,
    #[serde_as(as = "Vec<FixedString>")]
    #[serde(rename = "mints_burns.recipient")]
    pub mints_burns_recipient: Vec<Address>,
    #[serde_as(as = "Vec<Vec<FixedString>>")]
    #[serde(rename = "mints_burns.token")]
    pub mints_burns_tokens: Vec<Vec<Address>>,
    #[serde(rename = "mints_burns.amount")]
    pub mints_burns_amounts: Vec<Vec<u64>>,
    #[serde(rename = "gas_details.kind")]
    pub gas_details_kind: Vec<JitKind>,
    #[serde(rename = "gas_details.coinbase_transfer")]
    pub gas_details_coinbase_transfer: Vec<Option<u64>>,
    #[serde(rename = "gas_details.priority_fee")]
    pub gas_details_priority_fee: Vec<u64>,
    #[serde(rename = "gas_details.gas_used")]
    pub gas_details_gas_used: Vec<u64>,
    #[serde(rename = "gas_details.effective_gas_price")]
    pub gas_details_effective_gas_price: Vec<u64>,
}

impl SpecificMev for JitLiquidity {
    fn mev_type(&self) -> MevType {
        MevType::jit
    }

    fn mev_transaction_hashes(&self) -> Vec<H256> {
        vec![self.mint_tx_hash, self.burn_tx_hash]
    }

    fn bribe(&self) -> u64 {
        let gas_details = self
            .gas_details_kind
            .iter()
            .zip(self.gas_details_coinbase_transfer.clone())
            .collect::<Vec<_>>();

        let mut bribe = 0;

        gas_details
            .into_iter()
            .for_each(|(det_type, det_amt)| match det_type {
                JitKind::mint => bribe += det_amt.unwrap_or(0),
                JitKind::burn => bribe += det_amt.unwrap_or(0),
                _ => (),
            });

        bribe
    }

    fn priority_fee_paid(&self) -> u64 {
        let gas_details = self
            .gas_details_kind
            .iter()
            .zip(self.gas_details_priority_fee.clone())
            .collect::<Vec<_>>();

        let mut priority_fee = 0;

        gas_details
            .into_iter()
            .for_each(|(det_type, det_amt)| match det_type {
                JitKind::mint => priority_fee += det_amt,
                JitKind::burn => priority_fee += det_amt,
                _ => (),
            });

        priority_fee
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }
}

#[serde_as]
#[derive(Debug, Serialize, Row, Clone, Default)]
pub struct AtomicBackrun {
    #[serde_as(as = "FixedString")]
    pub tx_hash:          H256,
    #[serde(rename = "swaps.index")]
    pub swaps_index:      Vec<u16>,
    #[serde_as(as = "Vec<FixedString>")]
    #[serde(rename = "swaps.from")]
    pub swaps_from:       Vec<Address>,
    #[serde_as(as = "Vec<FixedString>")]
    #[serde(rename = "swaps.pool")]
    pub swaps_pool:       Vec<Address>,
    #[serde_as(as = "Vec<FixedString>")]
    #[serde(rename = "swaps.token_in")]
    pub swaps_token_in:   Vec<Address>,
    #[serde_as(as = "Vec<FixedString>")]
    #[serde(rename = "swaps.token_out")]
    pub swaps_token_out:  Vec<Address>,
    #[serde(rename = "swaps.amount_in")]
    pub swaps_amount_in:  Vec<u64>,
    #[serde(rename = "swaps.amount_out")]
    pub swaps_amount_out: Vec<u64>,
    #[serde(with = "gas_details_tuple")]
    pub gas_details:      GasDetails,
}

impl SpecificMev for AtomicBackrun {
    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }

    fn priority_fee_paid(&self) -> u64 {
        self.gas_details.priority_fee
    }

    fn bribe(&self) -> u64 {
        self.gas_details.coinbase_transfer.unwrap_or(0) as u64
    }

    fn mev_transaction_hashes(&self) -> Vec<H256> {
        vec![self.tx_hash]
    }

    fn mev_type(&self) -> MevType {
        MevType::backrun
    }
}

mod gas_details_tuple {
    use reth_primitives::U256;
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

    pub fn deserialize<'de, D>(deserializer: D) -> Result<GasDetails, D::Error>
    where
        D: Deserializer<'de>,
    {
        let tuple = <(u128, u64, u64, u64)>::deserialize(deserializer)?;
        Ok(GasDetails {
            coinbase_transfer:   Some(tuple.0),
            priority_fee:        tuple.1,
            gas_used:            tuple.2,
            effective_gas_price: tuple.3,
        })
    }
}
