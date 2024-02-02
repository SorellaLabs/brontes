use std::fmt::Debug;

use ::serde::ser::{SerializeStruct, Serializer};
use reth_primitives::B256;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use sorella_db_databases::clickhouse::{fixed_string::FixedString, DbRow};

use super::{Bundle, BundleData, BundleHeader, JitLiquidity, Mev, MevType, Sandwich};
#[allow(unused_imports)]
use crate::{
    display::utils::display_sandwich,
    normalized_actions::{NormalizedBurn, NormalizedLiquidation, NormalizedMint, NormalizedSwap},
    serde_primitives::vec_fixed_string,
    GasDetails,
};
use crate::{
    normalized_actions::{
        ClickhouseDoubleVecNormalizedSwap, ClickhouseVecNormalizedMintOrBurn,
        ClickhouseVecNormalizedMintOrBurnWithTxHash, ClickhouseVecNormalizedSwap,
    },
    tree::ClickhouseVecGasDetails,
};

#[serde_as]
#[derive(Debug, Deserialize, Clone, Default)]
pub struct JitLiquiditySandwich {
    pub frontrun_tx_hash:     Vec<B256>,
    pub frontrun_swaps:       Vec<Vec<NormalizedSwap>>,
    pub frontrun_mints:       Vec<Option<Vec<NormalizedMint>>>,
    pub frontrun_gas_details: Vec<GasDetails>,

    pub victim_swaps_tx_hashes:   Vec<Vec<B256>>,
    pub victim_swaps:             Vec<Vec<NormalizedSwap>>,
    pub victim_swaps_gas_details: Vec<GasDetails>,

    // Similar to frontrun fields, backrun fields are also vectors to handle multiple transactions.
    pub backrun_tx_hash:     B256,
    pub backrun_swaps:       Vec<NormalizedSwap>,
    pub backrun_burns:       Vec<NormalizedBurn>,
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
}

pub fn compose_sandwich_jit(mev: Vec<Bundle>) -> Bundle {
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
            _ => unreachable!(),
        }
    }

    let sandwich = sandwich.expect("Expected Sandwich MEV data");
    let jit = jit.expect("Expected JIT MEV data");
    let mut classified_sandwich =
        classified_sandwich.expect("Expected Classified MEV data for Sandwich");
    let jit_classified = jit_classified.expect("Expected Classified MEV data for JIT");

    let mut frontrun_mints: Vec<Option<Vec<NormalizedMint>>> =
        vec![None; sandwich.frontrun_tx_hash.len()];
    frontrun_mints
        .iter_mut()
        .enumerate()
        .for_each(|(idx, mint)| {
            if &sandwich.frontrun_tx_hash[idx] == &jit.frontrun_mint_tx_hash {
                *mint = Some(jit.frontrun_mints.clone())
            }
        });

    let mut backrun_burns: Vec<Option<Vec<NormalizedBurn>>> =
        vec![None; sandwich.frontrun_tx_hash.len()];
    backrun_burns
        .iter_mut()
        .enumerate()
        .for_each(|(idx, mint)| {
            if &sandwich.frontrun_tx_hash[idx] == &jit.backrun_burn_tx_hash {
                *mint = Some(jit.backrun_burns.clone())
            }
        });

    // sandwich.frontrun_swaps

    // Combine data from Sandwich and JitLiquidity into JitLiquiditySandwich
    let jit_sand = JitLiquiditySandwich {
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

    let sandwich_rev = classified_sandwich.bribe_usd + classified_sandwich.profit_usd;
    let jit_rev = jit_classified.bribe_usd + jit_classified.profit_usd;
    let jit_liq_profit = sandwich_rev + jit_rev - classified_sandwich.bribe_usd;

    // Compose token profits
    classified_sandwich
        .token_profits
        .compose(&jit_classified.token_profits);

    // Create new classified MEV data
    let new_classified = BundleHeader {
        tx_index:      classified_sandwich.tx_index,
        tx_hash:       *sandwich.frontrun_tx_hash.get(0).unwrap_or_default(),
        mev_type:      MevType::JitSandwich,
        block_number:  classified_sandwich.block_number,
        eoa:           jit_classified.eoa,
        mev_contract:  classified_sandwich.mev_contract,
        profit_usd:    jit_liq_profit,
        token_profits: classified_sandwich.token_profits,
        bribe_usd:     classified_sandwich.bribe_usd,
    };

    Bundle { header: new_classified, data: BundleData::JitSandwich(jit_sand) }
}

impl Serialize for JitLiquiditySandwich {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut ser_struct = serializer.serialize_struct("JitLiquiditySandwich", 34)?;

        // frontruns
        ser_struct.serialize_field(
            "frontrun_tx_hash",
            &FixedString::from(format!("{:?}", self.frontrun_tx_hash.first().unwrap_or_default())),
        )?;

        let frontrun_swaps: ClickhouseDoubleVecNormalizedSwap =
            (self.frontrun_tx_hash.clone(), self.frontrun_swaps.clone()).into();
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
            (self.frontrun_tx_hash.clone(), self.frontrun_mints.clone()).into();

        ser_struct.serialize_field("frontrun_mints.tx_hash", &frontrun_mints.tx_hash)?;
        ser_struct.serialize_field("frontrun_mints.trace_idx", &frontrun_mints.trace_index)?;
        ser_struct.serialize_field("frontrun_mints.from", &frontrun_mints.from)?;
        ser_struct.serialize_field("frontrun_mints.to", &frontrun_mints.to)?;
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
            (self.victim_swaps_tx_hashes.clone(), self.victim_swaps.clone()).into();
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
        let fixed_str_backrun_tx_hash = FixedString::from(format!("{:?}", &self.backrun_tx_hash));
        ser_struct.serialize_field("backrun_tx_hash", &fixed_str_backrun_tx_hash)?;

        let backrun_swaps: ClickhouseVecNormalizedSwap = self.backrun_swaps.clone().into();
        let backrun_tx_hash_repeated = vec![&self.backrun_tx_hash]
            .repeat(backrun_swaps.amount_in.len())
            .into_iter()
            .map(|tx| FixedString::from(format!("{:?}", &tx)))
            .collect::<Vec<_>>();

        ser_struct.serialize_field("backrun_swaps.tx_hash", &backrun_tx_hash_repeated)?;
        ser_struct.serialize_field("backrun_swaps.trace_idx", &backrun_swaps.trace_index)?;
        ser_struct.serialize_field("backrun_swaps.from", &backrun_swaps.from)?;
        ser_struct.serialize_field("backrun_swaps.recipient", &backrun_swaps.recipient)?;
        ser_struct.serialize_field("backrun_swaps.pool", &backrun_swaps.pool)?;
        ser_struct.serialize_field("backrun_swaps.token_in", &backrun_swaps.token_in)?;
        ser_struct.serialize_field("backrun_swaps.token_out", &backrun_swaps.token_out)?;
        ser_struct.serialize_field("backrun_swaps.amount_in", &backrun_swaps.amount_in)?;
        ser_struct.serialize_field("backrun_swaps.amount_out", &backrun_swaps.amount_out)?;

        let backrun_burns: ClickhouseVecNormalizedMintOrBurn = self.backrun_burns.clone().into();

        ser_struct.serialize_field("backrun_burns.tx_hash", &backrun_tx_hash_repeated)?;
        ser_struct.serialize_field("backrun_burns.trace_idx", &backrun_burns.trace_index)?;
        ser_struct.serialize_field("backrun_burns.from", &backrun_burns.from)?;
        ser_struct.serialize_field("backrun_burns.to", &backrun_burns.to)?;
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
        "frontrun_mints.to",
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
        "backrun_burns.to",
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
