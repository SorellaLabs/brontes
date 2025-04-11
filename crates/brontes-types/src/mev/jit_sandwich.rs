use std::fmt::Debug;

use ::serde::ser::{SerializeStruct, Serializer};
use ahash::HashSet;
use alloy_primitives::B256;
#[allow(unused)]
use clickhouse::row::*;
use redefined::Redefined;
use rkyv::{Archive, Deserialize as rDeserialize, Serialize as rSerialize};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

use super::{Bundle, BundleData, BundleHeader, JitLiquidity, Mev, MevType, Sandwich};
use crate::{
    db::redefined_types::primitives::*, normalized_actions::*, tree::ClickhouseVecGasDetails,
    Protocol,
};
#[allow(unused_imports)]
use crate::{
    display::utils::display_sandwich,
    normalized_actions::{NormalizedBurn, NormalizedLiquidation, NormalizedMint, NormalizedSwap},
    GasDetails,
};

#[serde_as]
#[derive(Debug, Deserialize, PartialEq, Clone, Default, Redefined)]
#[redefined_attr(derive(Debug, PartialEq, Clone, Serialize, rSerialize, rDeserialize, Archive))]
pub struct JitLiquiditySandwich {
    pub block_number:         u64,
    pub frontrun_tx_hash:     Vec<B256>,
    pub frontrun_swaps:       Vec<Vec<NormalizedSwap>>,
    pub frontrun_mints:       Vec<Option<Vec<NormalizedMint>>>,
    #[redefined(same_fields)]
    pub frontrun_gas_details: Vec<GasDetails>,

    pub victim_swaps_tx_hashes:   Vec<Vec<B256>>,
    pub victim_swaps:             Vec<Vec<NormalizedSwap>>,
    #[redefined(same_fields)]
    pub victim_swaps_gas_details: Vec<GasDetails>,

    // Similar to frontrun fields, backrun fields are also vectors to handle multiple transactions.
    pub backrun_tx_hash:     B256,
    pub backrun_swaps:       Vec<NormalizedSwap>,
    pub backrun_burns:       Vec<NormalizedBurn>,
    #[redefined(same_fields)]
    pub backrun_gas_details: GasDetails,
}

impl Mev for JitLiquiditySandwich {
    fn mev_type(&self) -> MevType {
        MevType::JitSandwich
    }

    fn total_gas_paid(&self) -> u128 {
        self.frontrun_gas_details
            .iter()
            .map(|gd| gd.gas_paid())
            .sum::<u128>()
            + self.backrun_gas_details.gas_paid()
    }

    fn total_priority_fee_paid(&self, base_fee: u128) -> u128 {
        self.frontrun_gas_details
            .iter()
            .map(|gd| gd.priority_fee_paid(base_fee))
            .sum::<u128>()
            + self.backrun_gas_details.priority_fee_paid(base_fee)
    }

    // Should always be on the backrun, but you never know
    fn bribe(&self) -> u128 {
        self.frontrun_gas_details
            .iter()
            .filter_map(|gd| gd.coinbase_transfer)
            .sum::<u128>()
            + self
                .backrun_gas_details
                .coinbase_transfer
                .unwrap_or_default()
    }

    fn mev_transaction_hashes(&self) -> Vec<B256> {
        let mut txs = self.frontrun_tx_hash.clone();
        txs.extend(self.victim_swaps_tx_hashes.iter().flatten().copied());
        txs.push(self.backrun_tx_hash);
        txs
    }

    fn protocols(&self) -> HashSet<Protocol> {
        let mut protocols: HashSet<Protocol> = self
            .frontrun_swaps
            .iter()
            .flatten()
            .map(|swap| swap.protocol)
            .collect();

        self.victim_swaps.iter().flatten().for_each(|swap| {
            protocols.insert(swap.protocol);
        });

        self.backrun_swaps.iter().for_each(|swap| {
            protocols.insert(swap.protocol);
        });

        protocols
    }
}

pub fn compose_sandwich_jit(mev: Vec<Bundle>) -> Option<Bundle> {
    let mut sandwich: Option<Sandwich> = None;
    let mut jit: Option<JitLiquidity> = None;
    let mut classified_sandwich: Option<BundleHeader> = None;
    let mut jit_classified: Option<BundleHeader> = None;

    for bundle in mev {
        match bundle.data {
            BundleData::Sandwich(s) => {
                sandwich = Some(s);
                classified_sandwich = Some(bundle.header);
            }
            BundleData::Jit(j) => {
                jit = Some(j);
                jit_classified = Some(bundle.header);
            }
            err => unreachable!("got bundle {err:?} in compose jit sandwich"),
        }
    }

    let sandwich = sandwich.expect("Expected Sandwich MEV data");
    let jit = jit.expect("Expected JIT MEV data");
    let classified_sandwich =
        classified_sandwich.expect("Expected Classified MEV data for Sandwich");
    let jit_classified = jit_classified.expect("Expected Classified MEV data for JIT");

    let mut frontrun_mints: Vec<Option<Vec<NormalizedMint>>> =
        vec![None; sandwich.frontrun_tx_hash.len()];
    frontrun_mints
        .iter_mut()
        .enumerate()
        .for_each(|(idx, mint)| {
            if sandwich.frontrun_tx_hash[idx] == jit.frontrun_mint_tx_hash {
                *mint = Some(jit.frontrun_mints.clone())
            }
        });

    let mut backrun_burns: Vec<Option<Vec<NormalizedBurn>>> =
        vec![None; sandwich.frontrun_tx_hash.len()];
    backrun_burns
        .iter_mut()
        .enumerate()
        .for_each(|(idx, mint)| {
            if sandwich.frontrun_tx_hash[idx] == jit.backrun_burn_tx_hash {
                *mint = Some(jit.backrun_burns.clone())
            }
        });

    // sandwich.frontrun_swaps

    // Combine data from Sandwich and JitLiquidity into JitLiquiditySandwich
    let jit_sand = JitLiquiditySandwich {
        block_number: sandwich.block_number,
        frontrun_tx_hash: sandwich.frontrun_tx_hash.clone(),
        frontrun_swaps: sandwich.frontrun_swaps,
        frontrun_mints,
        frontrun_gas_details: sandwich.frontrun_gas_details,
        victim_swaps_tx_hashes: sandwich.victim_swaps_tx_hashes,
        victim_swaps: sandwich.victim_swaps,
        victim_swaps_gas_details: sandwich.victim_swaps_gas_details,
        backrun_tx_hash: sandwich.backrun_tx_hash,
        backrun_swaps: sandwich.backrun_swaps,
        backrun_burns: jit.backrun_burns,
        backrun_gas_details: sandwich.backrun_gas_details,
    };

    // Create new classified MEV data
    let new_classified = BundleHeader {
        tx_index:              classified_sandwich.tx_index,
        tx_hash:               *sandwich.frontrun_tx_hash.first().unwrap_or_default(),
        mev_type:              MevType::JitSandwich,
        fund:                  classified_sandwich.fund,
        block_number:          classified_sandwich.block_number,
        eoa:                   jit_classified.eoa,
        mev_contract:          classified_sandwich.mev_contract,
        profit_usd:            classified_sandwich.profit_usd,
        balance_deltas:        classified_sandwich.balance_deltas,
        bribe_usd:             classified_sandwich.bribe_usd,
        no_pricing_calculated: classified_sandwich.no_pricing_calculated,
    };

    Some(Bundle { header: new_classified, data: BundleData::JitSandwich(jit_sand) })
}

impl Serialize for JitLiquiditySandwich {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut ser_struct = serializer.serialize_struct("JitLiquiditySandwich", 35)?;
        ser_struct.serialize_field("block_number", &self.block_number)?;

        // frontruns
        ser_struct.serialize_field(
            "frontrun_tx_hash",
            &format!("{:?}", self.frontrun_tx_hash.first().unwrap_or_default()),
        )?;

        let frontrun_swaps: ClickhouseDoubleVecNormalizedSwap =
            (self.frontrun_tx_hash.clone(), self.frontrun_swaps.clone())
                .try_into()
                .map_err(serde::ser::Error::custom)?;
        ser_struct.serialize_field("frontrun_swaps.tx_hash", &frontrun_swaps.tx_hash)?;
        ser_struct.serialize_field("frontrun_swaps.trace_idx", &frontrun_swaps.trace_index)?;
        ser_struct.serialize_field("frontrun_swaps.from", &frontrun_swaps.from)?;
        ser_struct.serialize_field("frontrun_swaps.recipient", &frontrun_swaps.recipient)?;
        ser_struct.serialize_field("frontrun_swaps.pool", &frontrun_swaps.pool)?;
        ser_struct.serialize_field("frontrun_swaps.token_in", &frontrun_swaps.token_in)?;
        ser_struct.serialize_field("frontrun_swaps.token_out", &frontrun_swaps.token_out)?;
        ser_struct.serialize_field("frontrun_swaps.amount_in", &frontrun_swaps.amount_in)?;
        ser_struct.serialize_field("frontrun_swaps.amount_out", &frontrun_swaps.amount_out)?;

        let frontrun_mints: ClickhouseVecNormalizedMintOrBurnWithTxHash =
            (self.frontrun_tx_hash.clone(), self.frontrun_mints.clone())
                .try_into()
                .map_err(serde::ser::Error::custom)?;

        ser_struct.serialize_field("frontrun_mints.tx_hash", &frontrun_mints.tx_hash)?;
        ser_struct.serialize_field("frontrun_mints.trace_idx", &frontrun_mints.trace_index)?;
        ser_struct.serialize_field("frontrun_mints.from", &frontrun_mints.from)?;
        ser_struct.serialize_field("frontrun_mints.pool", &frontrun_mints.pool)?;
        ser_struct.serialize_field("frontrun_mints.recipient", &frontrun_mints.recipient)?;
        ser_struct.serialize_field("frontrun_mints.tokens", &frontrun_mints.tokens)?;
        ser_struct.serialize_field("frontrun_mints.amounts", &frontrun_mints.amounts)?;

        let frontrun_gas_details: ClickhouseVecGasDetails =
            (self.frontrun_tx_hash.clone(), self.frontrun_gas_details.clone()).into();
        ser_struct
            .serialize_field("frontrun_gas_details.tx_hash", &frontrun_gas_details.tx_hash)?;
        ser_struct.serialize_field(
            "frontrun_gas_details.coinbase_transfer",
            &frontrun_gas_details.coinbase_transfer,
        )?;
        ser_struct.serialize_field(
            "frontrun_gas_details.priority_fee",
            &frontrun_gas_details.priority_fee,
        )?;
        ser_struct
            .serialize_field("frontrun_gas_details.gas_used", &frontrun_gas_details.gas_used)?;
        ser_struct.serialize_field(
            "frontrun_gas_details.effective_gas_price",
            &frontrun_gas_details.effective_gas_price,
        )?;

        // victims
        let victim_swaps: ClickhouseDoubleVecNormalizedSwap =
            (self.victim_swaps_tx_hashes.clone(), self.victim_swaps.clone())
                .try_into()
                .map_err(serde::ser::Error::custom)?;
        ser_struct.serialize_field("victim_swaps.tx_hash", &victim_swaps.tx_hash)?;
        ser_struct.serialize_field("victim_swaps.trace_idx", &victim_swaps.trace_index)?;
        ser_struct.serialize_field("victim_swaps.from", &victim_swaps.from)?;
        ser_struct.serialize_field("victim_swaps.recipient", &victim_swaps.recipient)?;
        ser_struct.serialize_field("victim_swaps.pool", &victim_swaps.pool)?;
        ser_struct.serialize_field("victim_swaps.token_in", &victim_swaps.token_in)?;
        ser_struct.serialize_field("victim_swaps.token_out", &victim_swaps.token_out)?;
        ser_struct.serialize_field("victim_swaps.amount_in", &victim_swaps.amount_in)?;
        ser_struct.serialize_field("victim_swaps.amount_out", &victim_swaps.amount_out)?;

        let victim_gas_details: ClickhouseVecGasDetails =
            (self.victim_swaps_tx_hashes.clone(), self.victim_swaps_gas_details.clone()).into();
        ser_struct.serialize_field("victim_gas_details.tx_hash", &victim_gas_details.tx_hash)?;
        ser_struct.serialize_field(
            "victim_gas_details.coinbase_transfer",
            &victim_gas_details.coinbase_transfer,
        )?;
        ser_struct
            .serialize_field("victim_gas_details.priority_fee", &victim_gas_details.priority_fee)?;
        ser_struct.serialize_field("victim_gas_details.gas_used", &victim_gas_details.gas_used)?;
        ser_struct.serialize_field(
            "victim_gas_details.effective_gas_price",
            &victim_gas_details.effective_gas_price,
        )?;

        // backrun
        let fixed_str_backrun_tx_hash = format!("{:?}", &self.backrun_tx_hash);
        ser_struct.serialize_field("backrun_tx_hash", &fixed_str_backrun_tx_hash)?;

        let backrun_swaps: ClickhouseVecNormalizedSwap = self
            .backrun_swaps
            .clone()
            .try_into()
            .map_err(serde::ser::Error::custom)?;
        let backrun_tx_hash_repeated_swaps = [&self.backrun_tx_hash]
            .repeat(backrun_swaps.amount_in.len())
            .into_iter()
            .map(|tx| format!("{:?}", &tx))
            .collect::<Vec<_>>();

        ser_struct.serialize_field("backrun_swaps.tx_hash", &backrun_tx_hash_repeated_swaps)?;
        ser_struct.serialize_field("backrun_swaps.trace_idx", &backrun_swaps.trace_index)?;
        ser_struct.serialize_field("backrun_swaps.from", &backrun_swaps.from)?;
        ser_struct.serialize_field("backrun_swaps.recipient", &backrun_swaps.recipient)?;
        ser_struct.serialize_field("backrun_swaps.pool", &backrun_swaps.pool)?;
        ser_struct.serialize_field("backrun_swaps.token_in", &backrun_swaps.token_in)?;
        ser_struct.serialize_field("backrun_swaps.token_out", &backrun_swaps.token_out)?;
        ser_struct.serialize_field("backrun_swaps.amount_in", &backrun_swaps.amount_in)?;
        ser_struct.serialize_field("backrun_swaps.amount_out", &backrun_swaps.amount_out)?;

        let backrun_burns: ClickhouseVecNormalizedMintOrBurn = self
            .backrun_burns
            .clone()
            .try_into()
            .map_err(serde::ser::Error::custom)?;

        let backrun_tx_hash_repeated_burns = [&self.backrun_tx_hash]
            .repeat(backrun_burns.pool.len())
            .into_iter()
            .map(|tx| format!("{:?}", &tx))
            .collect::<Vec<_>>();

        ser_struct.serialize_field("backrun_burns.tx_hash", &backrun_tx_hash_repeated_burns)?;
        ser_struct.serialize_field("backrun_burns.trace_idx", &backrun_burns.trace_index)?;
        ser_struct.serialize_field("backrun_burns.from", &backrun_burns.from)?;
        ser_struct.serialize_field("backrun_burns.pool", &backrun_burns.pool)?;
        ser_struct.serialize_field("backrun_burns.recipient", &backrun_burns.recipient)?;
        ser_struct.serialize_field("backrun_burns.tokens", &backrun_burns.tokens)?;
        ser_struct.serialize_field("backrun_burns.amounts", &backrun_burns.amounts)?;

        ser_struct
            .serialize_field("backrun_gas_details.tx_hash", &vec![fixed_str_backrun_tx_hash])?;
        ser_struct.serialize_field(
            "backrun_gas_details.coinbase_transfer",
            &vec![self.backrun_gas_details.coinbase_transfer],
        )?;
        ser_struct.serialize_field(
            "backrun_gas_details.priority_fee",
            &vec![self.backrun_gas_details.priority_fee],
        )?;
        ser_struct.serialize_field(
            "backrun_gas_details.gas_used",
            &vec![self.backrun_gas_details.gas_used],
        )?;
        ser_struct.serialize_field(
            "backrun_gas_details.effective_gas_price",
            &vec![self.backrun_gas_details.effective_gas_price],
        )?;

        ser_struct.end()
    }
}

impl DbRow for JitLiquiditySandwich {
    const COLUMN_NAMES: &'static [&'static str] = &[
        "block_number",
        "frontrun_tx_hash",
        "frontrun_swaps.tx_hash",
        "frontrun_swaps.trace_idx",
        "frontrun_swaps.from",
        "frontrun_swaps.recipient",
        "frontrun_swaps.pool",
        "frontrun_swaps.token_in",
        "frontrun_swaps.token_out",
        "frontrun_swaps.amount_in",
        "frontrun_swaps.amount_out",
        "frontrun_mints.tx_hash",
        "frontrun_mints.trace_idx",
        "frontrun_mints.from",
        "frontrun_mints.pool",
        "frontrun_mints.recipient",
        "frontrun_mints.tokens",
        "frontrun_mints.amounts",
        "frontrun_gas_details.tx_hash",
        "frontrun_gas_details.coinbase_transfer",
        "frontrun_gas_details.priority_fee",
        "frontrun_gas_details.gas_used",
        "frontrun_gas_details.effective_gas_price",
        "victim_swaps.tx_hash",
        "victim_swaps.trace_idx",
        "victim_swaps.from",
        "victim_swaps.recipient",
        "victim_swaps.pool",
        "victim_swaps.token_in",
        "victim_swaps.token_out",
        "victim_swaps.amount_in",
        "victim_swaps.amount_out",
        "victim_gas_details.tx_hash",
        "victim_gas_details.coinbase_transfer",
        "victim_gas_details.priority_fee",
        "victim_gas_details.gas_used",
        "victim_gas_details.effective_gas_price",
        "backrun_tx_hash",
        "backrun_swaps.tx_hash",
        "backrun_swaps.trace_idx",
        "backrun_swaps.from",
        "backrun_swaps.recipient",
        "backrun_swaps.pool",
        "backrun_swaps.token_in",
        "backrun_swaps.token_out",
        "backrun_swaps.amount_in",
        "backrun_swaps.amount_out",
        "backrun_burns.tx_hash",
        "backrun_burns.trace_idx",
        "backrun_burns.from",
        "backrun_burns.pool",
        "backrun_burns.recipient",
        "backrun_burns.tokens",
        "backrun_burns.amounts",
        "backrun_gas_details.tx_hash",
        "backrun_gas_details.coinbase_transfer",
        "backrun_gas_details.priority_fee",
        "backrun_gas_details.gas_used",
        "backrun_gas_details.effective_gas_price",
    ];
}
