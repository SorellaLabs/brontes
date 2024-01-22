use std::fmt::Debug;

use alloy_primitives::Address;
use dyn_clone::DynClone;
use redefined::{self_convert_redefined, RedefinedConvert};
use reth_primitives::B256;
use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use serde_with::serde_as;
use sorella_db_databases::{
    clickhouse,
    clickhouse::{fixed_string::FixedString, InsertRow, Row},
};
use strum::{Display, EnumIter};

use crate::{
    normalized_actions::{NormalizedBurn, NormalizedLiquidation, NormalizedMint, NormalizedSwap},
    serde_utils::primitives::vec_fixed_string,
    tree::GasDetails,
};

#[serde_as]
#[derive(Debug, Deserialize, Row, Clone, Default)]
pub struct MevBlock {
    pub block_hash: B256,
    pub block_number: u64,
    pub mev_count: u64,
    pub finalized_eth_price: f64,
    pub cumulative_gas_used: u128,
    pub cumulative_gas_paid: u128,
    pub total_bribe: u128,
    pub cumulative_mev_priority_fee_paid: u128,
    pub builder_address: Address,
    pub builder_eth_profit: f64,
    pub builder_finalized_profit_usd: f64,
    pub proposer_fee_recipient: Option<Address>,
    pub proposer_mev_reward: Option<u128>,
    pub proposer_finalized_profit_usd: Option<f64>,
    pub cumulative_mev_finalized_profit_usd: f64,
    pub possible_missed_arbs: Vec<B256>,
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Row, Clone, Default)]
pub struct BundleHeader {
    // can be multiple for sandwich
    pub block_number:         u64,
    pub mev_tx_index:         u64,
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

self_convert_redefined!(MevType);

#[derive(Debug, Deserialize, EnumIter, Clone, Default, Display)]
pub enum BundleData {
    Sandwich(Sandwich),
    AtomicBackrun(AtomicBackrun),
    JitSandwich(JitLiquiditySandwich),
    Jit(JitLiquidity),
    CexDex(CexDex),
    Liquidation(Liquidation),
    #[default]
    Unknown,
}

impl Mev for BundleData {
    fn mev_type(&self) -> MevType {
        match self {
            BundleData::Sandwich(m) => m.mev_type(),
            BundleData::AtomicBackrun(m) => m.mev_type(),
            BundleData::JitSandwich(m) => m.mev_type(),
            BundleData::Jit(m) => m.mev_type(),
            BundleData::CexDex(m) => m.mev_type(),
            BundleData::Liquidation(m) => m.mev_type(),
            BundleData::Unknown => MevType::Unknown,
        }
    }

    fn priority_fee_paid(&self) -> u128 {
        match self {
            BundleData::Sandwich(m) => m.priority_fee_paid(),
            BundleData::AtomicBackrun(m) => m.priority_fee_paid(),
            BundleData::JitSandwich(m) => m.priority_fee_paid(),
            BundleData::Jit(m) => m.priority_fee_paid(),
            BundleData::CexDex(m) => m.priority_fee_paid(),
            BundleData::Liquidation(m) => m.priority_fee_paid(),
            BundleData::Unknown => unimplemented!("calling priority_fee_paid() on unknown mev"),
        }
    }

    fn bribe(&self) -> u128 {
        match self {
            BundleData::Sandwich(m) => m.bribe(),
            BundleData::AtomicBackrun(m) => m.bribe(),
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
            BundleData::AtomicBackrun(m) => m.mev_transaction_hashes(),
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

impl From<AtomicBackrun> for BundleData {
    fn from(value: AtomicBackrun) -> Self {
        Self::AtomicBackrun(value)
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
/// - Frontrun Tx 1: [Etherscan Link](https://etherscan.io/tx/0x2a187ed5ba38cc3b857726df51ce99ee6e29c9bcaa02be1a328f99c3783b3303)
/// - Victim 1: [Etherscan Link](https://etherscan.io/tx/0x7325392f41338440f045cb1dba75b6099f01f8b00983e33cc926eb27aacd7e2d)
/// - Frontrun 2: [Etherscan Link](https://etherscan.io/tx/0xbcb8115fb54b7d6b0a0b0faf6e65fae02066705bd4afde70c780d4251a771428)
/// - Victim 2: [Etherscan Link](https://etherscan.io/tx/0x0b428553bc2ccc8047b0da46e6c1c1e8a338d9a461850fcd67ddb233f6984677)
/// - Backrun: [Etherscan Link](https://etherscan.io/tx/0xfb2ef488bf7b6ad09accb126330837198b0857d2ea0052795af520d470eb5e1d)
#[serde_as]
#[derive(Debug, Deserialize, Clone, Default)]
pub struct Sandwich {
    /// Transaction hashes of the frontrunning transactions.
    /// Supports multiple transactions for complex sandwich scenarios.
    pub frontrun_tx_hash:         Vec<B256>,
    /// Swaps executed in each frontrunning transaction.
    /// Nested vectors represent multiple swaps within each transaction.
    pub frontrun_swaps:           Vec<Vec<NormalizedSwap>>,
    /// Gas details for each frontrunning transaction.
    pub frontrun_gas_details:     Vec<GasDetails>,
    /// Transaction hashes of the victim transactions, logically grouped by
    /// their corresponding frontrunning transaction. Each outer vector
    /// index corresponds to a frontrun transaction, grouping victims targeted
    /// by that specific frontrun.
    pub victim_swaps_tx_hashes:   Vec<Vec<B256>>,
    /// Swaps executed by victims, grouped to align with corresponding
    /// frontrunning transactions.
    pub victim_swaps:             Vec<Vec<NormalizedSwap>>,
    /// Gas details for each victim transaction.
    pub victim_swaps_gas_details: Vec<GasDetails>,
    /// Transaction hashes of the backrunning transactions.
    pub backrun_tx_hash:          B256,
    /// Swaps executed in each backrunning transaction.
    pub backrun_swaps:            Vec<NormalizedSwap>,
    /// Gas details for each backrunning transaction.
    pub backrun_gas_details:      GasDetails,
}
pub fn compose_sandwich_jit(mev: Vec<(BundleHeader, BundleData)>) -> (BundleHeader, BundleData) {
    let mut sandwich: Option<Sandwich> = None;
    let mut jit: Option<JitLiquidity> = None;
    let mut classified_sandwich: Option<BundleHeader> = None;
    let mut jit_classified: Option<BundleHeader> = None;

    for (classified, mev_data) in mev {
        match mev_data {
            BundleData::Sandwich(s) => {
                sandwich = Some(s);
                classified_sandwich = Some(classified);
            }
            BundleData::Jit(j) => {
                jit = Some(j);
                jit_classified = Some(classified);
            }
            _ => unreachable!(),
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

    let sandwich_rev =
        classified_sandwich.finalized_bribe_usd + classified_sandwich.finalized_profit_usd;
    let jit_rev = jit_classified.finalized_bribe_usd + jit_classified.finalized_profit_usd;
    let jit_liq_profit = sandwich_rev + jit_rev - classified_sandwich.finalized_bribe_usd;

    // Create new classified MEV data
    let new_classified = BundleHeader {
        mev_tx_index:         classified_sandwich.mev_tx_index,
        tx_hash:              *sandwich.frontrun_tx_hash.get(0).unwrap_or_default(),
        mev_type:             MevType::JitSandwich,
        block_number:         classified_sandwich.block_number,
        eoa:                  jit_classified.eoa,
        mev_contract:         classified_sandwich.mev_contract,
        mev_profit_collector: classified_sandwich.mev_profit_collector,
        finalized_bribe_usd:  classified_sandwich.finalized_bribe_usd,
        finalized_profit_usd: jit_liq_profit,
    };

    (new_classified, BundleData::JitSandwich(jit_sand))
}

impl Mev for Sandwich {
    fn mev_type(&self) -> MevType {
        MevType::Sandwich
    }

    fn priority_fee_paid(&self) -> u128 {
        self.frontrun_gas_details
            .iter()
            .map(|gd| gd.gas_paid())
            .sum::<u128>()
            + self.backrun_gas_details.gas_paid()
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
        txs.push(self.backrun_tx_hash);
        txs
    }
}

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

    fn priority_fee_paid(&self) -> u128 {
        self.frontrun_gas_details
            .iter()
            .map(|gd| gd.gas_paid())
            .sum::<u128>()
            + self.backrun_gas_details.gas_paid()
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
        txs.push(self.backrun_tx_hash);
        txs
    }
}

#[derive(
    Debug,
    Serialize_repr,
    Deserialize_repr,
    rkyv::Serialize,
    rkyv::Deserialize,
    rkyv::Archive,
    PartialEq,
    Eq,
    Hash,
    EnumIter,
    Clone,
    Copy,
)]
#[repr(u8)]
#[allow(non_camel_case_types)]
#[serde(rename_all = "lowercase")]
pub enum PriceKind {
    Cex = 0,
    Dex = 1,
}

self_convert_redefined!(PriceKind);

#[serde_as]
#[derive(Debug, Deserialize, Clone, Default)]
pub struct CexDex {
    pub tx_hash:        B256,
    pub swaps:          Vec<NormalizedSwap>,
    pub gas_details:    GasDetails,
    pub prices_kind:    Vec<PriceKind>,
    pub prices_address: Vec<Address>,
    pub prices_price:   Vec<f64>,
}
impl Mev for CexDex {
    fn mev_type(&self) -> MevType {
        MevType::CexDex
    }

    fn priority_fee_paid(&self) -> u128 {
        self.gas_details.gas_paid()
    }

    fn mev_transaction_hashes(&self) -> Vec<B256> {
        vec![self.tx_hash]
    }

    fn bribe(&self) -> u128 {
        self.gas_details.coinbase_transfer.unwrap_or(0)
    }
}

#[serde_as]
#[derive(Debug, Deserialize, Clone, Default)]
pub struct Liquidation {
    pub liquidation_tx_hash: B256,
    pub trigger:             B256,
    pub liquidation_swaps:   Vec<NormalizedSwap>,
    pub liquidations:        Vec<NormalizedLiquidation>,
    pub gas_details:         GasDetails,
}

impl Mev for Liquidation {
    fn mev_type(&self) -> MevType {
        MevType::Liquidation
    }

    fn mev_transaction_hashes(&self) -> Vec<B256> {
        vec![self.liquidation_tx_hash]
    }

    fn priority_fee_paid(&self) -> u128 {
        self.gas_details.gas_paid()
    }

    fn bribe(&self) -> u128 {
        self.gas_details.coinbase_transfer.unwrap_or(0)
    }
}

#[serde_as]
#[derive(Debug, Deserialize, Clone, Default)]
pub struct JitLiquidity {
    pub frontrun_mint_tx_hash: B256,
    pub frontrun_mints: Vec<NormalizedMint>,
    pub frontrun_mint_gas_details: GasDetails,
    pub victim_swaps_tx_hashes: Vec<B256>,
    pub victim_swaps: Vec<Vec<NormalizedSwap>>,
    pub victim_swaps_gas_details_tx_hashes: Vec<B256>,
    pub victim_swaps_gas_details: Vec<GasDetails>,
    pub backrun_burn_tx_hash: B256,
    pub backrun_burns: Vec<NormalizedBurn>,
    pub backrun_burn_gas_details: GasDetails,
}

impl Mev for JitLiquidity {
    fn mev_type(&self) -> MevType {
        MevType::Jit
    }

    fn mev_transaction_hashes(&self) -> Vec<B256> {
        vec![self.frontrun_mint_tx_hash, self.backrun_burn_tx_hash]
    }

    fn bribe(&self) -> u128 {
        self.frontrun_mint_gas_details
            .coinbase_transfer
            .unwrap_or(0)
            + self.backrun_burn_gas_details.coinbase_transfer.unwrap_or(0)
    }

    fn priority_fee_paid(&self) -> u128 {
        self.frontrun_mint_gas_details.gas_paid() + self.backrun_burn_gas_details.gas_paid()
    }
}

#[serde_as]
#[derive(Debug, Deserialize, Clone, Default)]
pub struct AtomicBackrun {
    pub tx_hash:     B256,
    pub swaps:       Vec<NormalizedSwap>,
    pub gas_details: GasDetails,
}

impl Mev for AtomicBackrun {
    fn priority_fee_paid(&self) -> u128 {
        self.gas_details.gas_paid()
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

#[cfg(test)]
mod tests {

    use std::{any::Any, str::FromStr};

    use serde::Serialize;
    use sorella_db_databases::{
        clickhouse::{db::ClickhouseClient, DbRow},
        tables::{DatabaseTables, FromDatabaseTables},
        Database,
    };

    use super::*;

    fn spawn_db() -> ClickhouseClient {
        ClickhouseClient::default()
    }

    #[tokio::test]
    async fn test_db_mev_block() {
        let test_block = MevBlock::default();

        let db: ClickhouseClient = spawn_db();

        db.insert_one(&test_block, DatabaseTables::MevBlocks)
            .await
            .unwrap();

        let delete_query = format!(
            "DELETE FROM {} where block_hash = ? and block_number = ?",
            db.to_table_string(DatabaseTables::MevBlocks)
        );
        db.execute_remote(
            &delete_query,
            &(format!("{:?}", test_block.block_hash), test_block.block_number),
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_db_classified_mev() {
        let test_mev = BundleHeader::default();

        let db = spawn_db();

        db.insert_one(&test_mev, DatabaseTables::BundleHeader)
            .await
            .unwrap();

        let delete_query = &format!(
            "DELETE FROM {} where tx_hash = ? and block_number = ?",
            db.to_table_string(DatabaseTables::BundleHeader)
        );

        db.execute_remote(
            &delete_query,
            &(format!("{:?}", test_mev.tx_hash), test_mev.block_number),
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_db_sandwhich() {
        let test_mev = Sandwich::default();
        let db = spawn_db();

        db.insert_one(&test_mev, DatabaseTables::Sandwich)
            .await
            .unwrap();

        let delete_query = format!(
            "DELETE FROM {} where frontrun_tx_hash = ? and backrun_tx_hash = ?",
            db.to_table_string(DatabaseTables::Sandwich)
        );
        db.execute_remote(
            &delete_query,
            &(
                format!("{:?}", test_mev.frontrun_tx_hash),
                format!("{:?}", test_mev.backrun_tx_hash),
            ),
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_db_jit_sandwhich() {
        let test_mev = JitLiquiditySandwich::default();

        let db = spawn_db();

        db.insert_one(&test_mev, DatabaseTables::JitSandwich)
            .await
            .unwrap();

        let delete_query = format!(
            "DELETE FROM {} where frontrun_tx_hash = ? and backrun_tx_hash = ?",
            db.to_table_string(DatabaseTables::JitSandwich)
        );

        db.execute_remote(
            &delete_query,
            &(
                format!("{:?}", test_mev.frontrun_tx_hash),
                format!("{:?}", test_mev.backrun_tx_hash),
            ),
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_db_jit() {
        let mut test_mev: JitLiquidity = JitLiquidity::default();
        test_mev.frontrun_mints.push(Default::default());
        test_mev.backrun_burn_gas_details.coinbase_transfer = None;
        test_mev.backrun_burns.iter_mut().for_each(|burn| {
            burn.token = vec![
                Address::from_str("0xb17548c7b510427baac4e267bea62e800b247173").unwrap(),
                Address::from_str("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2").unwrap(),
            ];
            burn.from = Default::default();
            burn.to = Default::default();
            burn.recipient = Default::default();
            burn.trace_index = Default::default();
            burn.amount = vec![Default::default()];
        });

        let db = spawn_db();

        db.insert_one(&test_mev, DatabaseTables::Jit).await.unwrap();

        let delete_query = format!(
            "DELETE FROM {} where frontrun_mint_tx_hash = ? and backrun_burn_tx_hash = ?",
            db.to_table_string(DatabaseTables::Jit)
        );

        db.execute_remote(
            &delete_query,
            &(
                format!("{:?}", test_mev.frontrun_mint_tx_hash),
                format!("{:?}", test_mev.backrun_burn_tx_hash),
            ),
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn test_db_liquidation() {
        let test_mev = Liquidation::default();

        let db = spawn_db();

        db.insert_one(&test_mev, DatabaseTables::Liquidations)
            .await
            .unwrap();

        let delete_query = format!(
            "DELETE FROM {} where liquidation_tx_hash = ?",
            db.to_table_string(DatabaseTables::Liquidations)
        );
        db.execute_remote(&delete_query, &(format!("{:?}", test_mev.liquidation_tx_hash)))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_db_atomic_backrun() {
        let test_mev = AtomicBackrun::default();

        let db = spawn_db();

        db.insert_one(&test_mev, DatabaseTables::AtomicBackrun)
            .await
            .unwrap();

        let delete_query = format!(
            "DELETE FROM {} where tx_hash = ?",
            db.to_table_string(DatabaseTables::AtomicBackrun)
        );
        db.execute_remote(&delete_query, &(format!("{:?}", test_mev.tx_hash)))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_db_cex_dex() {
        let test_mev = CexDex::default();

        let db = spawn_db();

        db.insert_one(&test_mev, DatabaseTables::CexDex)
            .await
            .unwrap();

        let delete_query =
            format!("DELETE FROM {} where tx_hash = ?", db.to_table_string(DatabaseTables::CexDex));
        db.execute_remote(&delete_query, &(format!("{:?}", test_mev.tx_hash)))
            .await
            .unwrap();
    }
}
