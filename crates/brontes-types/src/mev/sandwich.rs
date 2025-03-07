use std::fmt::Debug;

use ::clickhouse::DbRow;
use ::serde::ser::{SerializeStruct, Serializer};
use ahash::HashSet;
use alloy_primitives::{Address, B256};
use malachite::Rational;
use redefined::Redefined;
use rkyv::{Archive, Deserialize as rDeserialize, Serialize as rSerialize};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

use super::{Mev, MevType};
use crate::{
    db::{redefined_types::primitives::*, token_info::TokenInfoWithAddress},
    normalized_actions::*,
    ClickhouseVecGasDetails, Protocol,
};
#[allow(unused_imports)]
use crate::{
    display::utils::display_sandwich,
    normalized_actions::{
        ClickhouseDoubleVecNormalizedSwap, ClickhouseVecNormalizedSwap, NormalizedBurn,
        NormalizedLiquidation, NormalizedMint, NormalizedSwap,
    },
    GasDetails,
};

/// Represents various MEV sandwich attack strategies, including standard
/// sandwiches and more complex variations like the "Big Mac Sandwich."
///
/// The `Sandwich` struct is designed to be versatile, accommodating a range of
/// sandwich attack scenarios. While a standard sandwich attack typically
/// involves a single frontrunning and backrunning transaction around a victim's
/// trade, more complex variations can involve multiple frontrunning and
/// backrunning transactions targeting several victims with different slippage
/// tolerances.
///
/// The structure of this struct is generalized to support these variations. For
/// example, the "Big Mac Sandwich" is one such complex scenario where a bot
/// exploits multiple victims in a sequence of transactions, each with different
/// slippage tolerances. This struct can capture the details of both simple and
/// complex sandwich strategies, making it a comprehensive tool for MEV
/// analysis.
///
/// Example of a Complex Sandwich Attack ("Big Mac Sandwich") Transaction
/// Sequence:
/// Represents various MEV sandwich attack strategies, including standard
/// sandwiches and more complex variations like the "Big Mac Sandwich."

///
/// Example of a Complex Sandwich Attack ("Big Mac Sandwich") Transaction
/// Sequence:
/// - Frontrun Tx 1: [Etherscan Link](https://etherscan.io/tx/0x2a187ed5ba38cc3b857726df51ce99ee6e29c9bcaa02be1a328f99c3783b3303)
/// - Victim 1: [Etherscan Link](https://etherscan.io/tx/0x7325392f41338440f045cb1dba75b6099f01f8b00983e33cc926eb27aacd7e2d)
/// - Frontrun 2: [Etherscan Link](https://etherscan.io/tx/0xbcb8115fb54b7d6b0a0b0faf6e65fae02066705bd4afde70c780d4251a771428)
/// - Victim 2: [Etherscan Link](https://etherscan.io/tx/0x0b428553bc2ccc8047b0da46e6c1c1e8a338d9a461850fcd67ddb233f6984677)
/// - Backrun: [Etherscan Link](https://etherscan.io/tx/0xfb2ef488bf7b6ad09accb126330837198b0857d2ea0052795af520d470eb5e1d)
#[serde_as]
#[derive(Debug, Deserialize, PartialEq, Clone, Default, Redefined)]
#[redefined_attr(derive(Debug, PartialEq, Clone, Serialize, rSerialize, rDeserialize, Archive))]
pub struct Sandwich {
    pub block_number:             u64,
    /// Transaction hashes of the frontrunning transactions.
    /// Supports multiple transactions for complex sandwich scenarios.
    pub frontrun_tx_hash:         Vec<B256>,
    /// Swaps executed in each frontrunning transaction.
    /// Nested vectors represent multiple swaps within each transaction.
    pub frontrun_swaps:           Vec<Vec<NormalizedSwap>>,
    /// Gas details for each frontrunning transaction.
    #[redefined(same_fields)]
    pub frontrun_gas_details:     Vec<GasDetails>,
    /// Transaction hashes of the victim transactions, logically grouped by
    /// their corresponding frontrunning transaction. Each outer vector
    /// index corresponds to a frontrun transaction, grouping victims targeted
    /// by that specific frontrun.
    pub victim_swaps_tx_hashes:   Vec<Vec<B256>>,
    /// Swaps executed by victims, each outer vector corresponds to a victim
    /// transaction.
    pub victim_swaps:             Vec<Vec<NormalizedSwap>>,
    /// Gas details for each victim transaction.
    #[redefined(same_fields)]
    pub victim_swaps_gas_details: Vec<GasDetails>,
    /// Transaction hashes of the backrunning transactions.
    pub backrun_tx_hash:          B256,
    /// Swaps executed in each backrunning transaction.
    pub backrun_swaps:            Vec<NormalizedSwap>,
    /// Gas details for each backrunning transaction.
    #[redefined(same_fields)]
    pub backrun_gas_details:      GasDetails,
}

/// calcuation for the loss per user
#[derive(Debug, Deserialize, PartialEq, Clone, Default)]
pub struct VictimLossAmount {
    pub tx_hash:           B256,
    pub vicitim_eoa:       Address,
    pub token:             TokenInfoWithAddress,
    pub token_amount_lost: Rational,
    /// is zero if we don't have a price for the given token
    pub amount_lost_usd:   Rational,
}

impl Mev for Sandwich {
    fn mev_type(&self) -> MevType {
        MevType::Sandwich
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

impl Serialize for Sandwich {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut ser_struct = serializer.serialize_struct("Sandwich", 35)?;
        ser_struct.serialize_field("block_number", &self.block_number)?;

        // frontrun
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
        let backrun_tx_hash_repeated = [&self.backrun_tx_hash]
            .repeat(backrun_swaps.amount_in.len())
            .into_iter()
            .map(|tx| format!("{:?}", &tx))
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

impl DbRow for Sandwich {
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
        "backrun_gas_details.tx_hash",
        "backrun_gas_details.coinbase_transfer",
        "backrun_gas_details.priority_fee",
        "backrun_gas_details.gas_used",
        "backrun_gas_details.effective_gas_price",
    ];
}
