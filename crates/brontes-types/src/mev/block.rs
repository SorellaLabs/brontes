use std::{
    fmt::{self, Debug},
    ops::Add,
};

use alloy_primitives::Address;
#[allow(unused)]
use clickhouse::{fixed_string::FixedString, row::*, Row};
use colored::Colorize;
use indoc::indoc;
use redefined::{self_convert_redefined, Redefined};
use reth_primitives::B256;
use rkyv::{Archive, Deserialize as rDeser, Serialize as rSer};
use serde::{ser::SerializeStruct, Deserialize, Serialize};
use serde_with::serde_as;

use super::MevType;
use crate::{
    db::redefined_types::primitives::{AddressRedefined, B256Redefined},
    display::utils::format_etherscan_address_url,
    ToFloatNearest, ToScaledRational,
};
#[allow(unused_imports)]
use crate::{
    display::utils::display_sandwich,
    normalized_actions::{NormalizedBurn, NormalizedLiquidation, NormalizedMint, NormalizedSwap},
    GasDetails,
};

#[serde_as]
#[derive(Debug, Deserialize, PartialEq, Clone, Default, Redefined)]
#[redefined_attr(derive(Debug, PartialEq, Clone, Serialize, rSer, rDeser, Archive))]
pub struct MevBlock {
    pub block_hash:                  B256,
    pub block_number:                u64,
    #[redefined(same_fields)]
    pub mev_count:                   MevCount,
    pub eth_price:                   f64,
    pub total_gas_used:              u128,
    pub total_priority_fee:          u128,
    pub total_bribe:                 u128,
    pub total_mev_bribe:             u128,
    pub total_mev_priority_fee_paid: u128,
    pub builder_address:             Address,
    pub builder_name:                Option<String>,
    pub builder_eth_profit:          f64,
    pub builder_profit_usd:          f64,
    // Builder MEV profit from their vertically integrated searchers (in USD)
    pub builder_mev_profit_usd:      f64,
    // Bribes paid to the builder by their own searchers
    pub builder_searcher_bribes:     u128,
    // Bribes paid to the builder by their own searchers (in USD)
    pub builder_searcher_bribes_usd: f64,
    pub builder_sponsorship_amount:  u128,
    pub ultrasound_bid_adjusted:     bool,
    pub proposer_fee_recipient:      Option<Address>,
    pub proposer_mev_reward:         Option<u128>,
    pub proposer_profit_usd:         Option<f64>,
    pub total_mev_profit_usd:        f64,
    pub possible_mev:                PossibleMevCollection,
}

impl fmt::Display for MevBlock {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let ascii_header = indoc! {r#"
                     ___  ___            ______ _            _                         
                     |  \/  |            | ___ \ |          | |                        
 ______ ______ ______| .  . | _____   __ | |_/ / | ___   ___| | ________ ______ ______ 
|______|______|______| |\/| |/ _ \ \ / / | ___ \ |/ _ \ / __| |/ /______|______|______|
                     | |  | |  __/\ V /  | |_/ / | (_) | (__|   <                      
                     \_|  |_/\___| \_/   \____/|_|\___/ \___|_|\_\                     
        
        "#};

        for line in ascii_header.lines() {
            writeln!(f, "{}", line.green())?;
        }

        let block_value = (self.total_priority_fee + self.total_bribe)
            .to_scaled_rational(18)
            .to_float();

        writeln!(f, "{} {}", "Block Number:".bold(), self.block_number)?;
        // Block value section
        writeln!(f, "\n{}: {:.6} ETH", "Block Value:".bold().red().underline(), block_value)?;

        // Mev
        writeln!(f, "\n{}", "Mev:".bold().red().underline())?;
        writeln!(f, "{}", self.mev_count.to_string().bold())?;
        writeln!(
            f,
            "  - {} {}",
            "Total MEV Profit (USD):".purple().bold(),
            format_profit(self.total_mev_profit_usd)
        )?;

        writeln!(f, "  - {}", "Gas:".bold().underline())?;
        writeln!(
            f,
            "    - {} {:.6} ETH ({:.2}% MEV)",
            "Total Builder Tips:".bold(),
            self.total_bribe as f64 * 1e-18,
            (self.total_mev_bribe as f64 * 1e-18) / (self.total_bribe as f64 * 1e-18) * 100.0
        )?;
        writeln!(
            f,
            "    - {} {:.6} ETH ({:.2}% MEV)",
            "Total Priority Fee:".bold(),
            self.total_mev_priority_fee_paid as f64 * 1e-18,
            (self.total_mev_priority_fee_paid as f64 * 1e-18)
                / (self.total_priority_fee as f64 * 1e-18)
                * 100.0
        )?;

        // Builder PnL
        writeln!(f, "\n{}", "Builder PnL:".bold().red().underline())?;

        let builder_profit_color = if self.builder_profit_usd < 0.0 { "red" } else { "green" };
        writeln!(
            f,
            "  - {} {:.9} ETH ({:.6} USD)",
            "Builder Profit:".bold(),
            format!("{:.6}", self.builder_eth_profit).color(builder_profit_color),
            format_profit(self.builder_profit_usd).color(builder_profit_color)
        )?;

        let builder_mev_profit_color =
            if self.builder_mev_profit_usd < 0.0 { "red" } else { "green" };
        writeln!(
            f,
            "  - {}: {:.6} USD",
            "Builder MEV Profit".bold().purple(),
            format!("{:.6}", self.builder_mev_profit_usd).color(builder_mev_profit_color)
        )?;

        writeln!(
            f,
            "  - {} {:.6} ETH ({:.2}% of Builder Profit)",
            "VI Bribes:".bold(),
            self.builder_searcher_bribes as f64 * 1e-18,
            self.builder_searcher_bribes as f64 * 1e-18 / self.builder_eth_profit
        )?;
        writeln!(
            f,
            "  - {} {}",
            "Builder Address::".bold(),
            format_etherscan_address_url(&self.builder_address)
        )?;

        // Proposer section
        writeln!(f, "{}", "Proposer:".bold().red().underline())?;

        if self.proposer_fee_recipient.is_none()
            || self.proposer_mev_reward.is_none()
            || self.proposer_profit_usd.is_none()
        {
            writeln!(f, "{}", "  - Isn't an MEV boost block".bold().red().underline())?;
        } else {
            writeln!(f, "  - Proposer Fee Recipient: {:?}", self.proposer_fee_recipient.unwrap())?;
            writeln!(
                f,
                "  - Proposer MEV Reward: {:.6} ETH",
                format!("{:.6}", self.proposer_mev_reward.unwrap() as f64 / 10f64.powf(18.0))
                    .green()
            )?;
            writeln!(
                f,
                "  - Proposer Finalized Profit (USD): {}",
                format_profit(self.proposer_profit_usd.unwrap()).green()
            )?;
        }

        writeln!(f, "\n{}: {}", "Missed Mev".bold().red().underline(), self.possible_mev)?;

        Ok(())
    }
}

// Helper function to format profit values
fn format_profit(value: f64) -> String {
    if value < 0.0 {
        format!("-${:.2}", value.abs())
    } else {
        format!("${:.2}", value)
    }
}

#[serde_as]
#[derive(Debug, Deserialize, PartialEq, Serialize, Row, Clone, Default, rDeser, rSer, Archive)]
pub struct MevCount {
    pub bundle_count:         u64,
    pub sandwich_count:       Option<u64>,
    pub cex_dex_trade_count:  Option<u64>,
    pub cex_dex_quote_count:  Option<u64>,
    pub cex_dex_rfq_count:    Option<u64>,
    pub jit_cex_dex_count:    Option<u64>,
    pub jit_count:            Option<u64>,
    pub jit_sandwich_count:   Option<u64>,
    pub atomic_backrun_count: Option<u64>,
    pub liquidation_count:    Option<u64>,
    pub searcher_tx_count:    Option<u64>,
}

impl MevCount {
    pub fn increment_count(&mut self, mev_type: MevType) {
        self.bundle_count += 1;
        match mev_type {
            MevType::CexDexTrades => {
                self.cex_dex_trade_count = Some(self.cex_dex_trade_count.unwrap_or_default().add(1))
            }
            MevType::CexDexQuotes => {
                self.cex_dex_quote_count = Some(self.cex_dex_quote_count.unwrap_or_default().add(1))
            }
            MevType::Sandwich => {
                self.sandwich_count = Some(self.sandwich_count.unwrap_or_default().add(1))
            }
            MevType::AtomicArb => {
                self.atomic_backrun_count =
                    Some(self.atomic_backrun_count.unwrap_or_default().add(1))
            }
            MevType::Jit => self.jit_count = Some(self.jit_count.unwrap_or_default().add(1)),
            MevType::JitSandwich => {
                self.jit_sandwich_count = Some(self.jit_sandwich_count.unwrap_or_default().add(1))
            }
            MevType::Liquidation => {
                self.liquidation_count = Some(self.liquidation_count.unwrap_or_default().add(1))
            }
            MevType::SearcherTx => {
                self.searcher_tx_count = Some(self.searcher_tx_count.unwrap_or_default().add(1))
            }
            MevType::JitCexDex => {
                self.jit_cex_dex_count = Some(self.jit_cex_dex_count.unwrap_or_default().add(1))
            }
            _ => {}
        }
    }
}
self_convert_redefined!(MevCount);

impl fmt::Display for MevCount {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "  - MEV Count: {}", self.bundle_count.to_string().bold())?;

        if let Some(count) = self.sandwich_count {
            writeln!(f, "    - Sandwich: {}", count.to_string().bold())?;
        }
        if let Some(count) = self.cex_dex_trade_count {
            writeln!(f, "    - Trade Cex-Dex: {}", count.to_string().bold())?;
        }
        if let Some(count) = self.cex_dex_quote_count {
            writeln!(f, "    - Quote Cex-Dex: {}", count.to_string().bold())?;
        }
        if let Some(count) = self.jit_count {
            writeln!(f, "    - Jit: {}", count.to_string().bold())?;
        }
        if let Some(count) = self.jit_cex_dex_count {
            writeln!(f, "    - Jit Cex-Dex: {}", count.to_string().bold())?;
        }
        if let Some(count) = self.jit_sandwich_count {
            writeln!(f, "    - Jit Sandwich: {}", count.to_string().bold())?;
        }
        if let Some(count) = self.atomic_backrun_count {
            writeln!(f, "    - Atomic Backrun: {}", count.to_string().bold())?;
        }
        if let Some(count) = self.liquidation_count {
            writeln!(f, "    - Liquidation: {}", count.to_string().bold())?;
        }
        if let Some(count) = self.searcher_tx_count {
            writeln!(f, "    - Searcher TXs: {}", count.to_string().bold())?;
        }

        Ok(())
    }
}

#[serde_as]
#[derive(Debug, Deserialize, PartialEq, Row, Clone, Default, Redefined)]
#[redefined_attr(derive(Debug, PartialEq, Clone, Serialize, rSer, rDeser, Archive))]
pub struct PossibleMevCollection(pub Vec<PossibleMev>);

impl fmt::Display for PossibleMevCollection {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(
            f,
            "{}",
            format!("Found {} possible MEV Transactions that we did not classify", self.0.len())
                .bright_yellow()
        )?;
        for possible_mev in self.0.iter() {
            writeln!(
                f,
                "    {}",
                format!("------ Transaction {} ------", possible_mev.tx_idx).purple()
            )?;
            writeln!(f, "    {}", possible_mev)?;
        }
        Ok(())
    }
}

impl fmt::Display for PossibleMev {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let eth_paid = self.gas_details.gas_paid() as f64 * 1e-18;
        let tx_url = format!("https://etherscan.io/tx/{:?}", self.tx_hash);
        writeln!(f, "        Paid {} Eth for inclusion", eth_paid.to_string().bold().green())?;
        write!(f, "{}", self.triggers)?;
        writeln!(f, "        Etherscan: {}", tx_url.underline())
    }
}

#[serde_as]
#[derive(Debug, Deserialize, PartialEq, Row, Clone, Default, Redefined)]
#[redefined_attr(derive(Debug, PartialEq, Clone, Serialize, rSer, rDeser, Archive))]
pub struct PossibleMev {
    pub tx_hash:     B256,
    pub tx_idx:      u64,
    #[redefined(same_fields)]
    pub gas_details: GasDetails,
    #[redefined(same_fields)]
    pub triggers:    PossibleMevTriggers,
}

#[serde_as]
#[derive(Debug, PartialEq, Deserialize, Row, Clone, Default, Serialize, rSer, rDeser, Archive)]
pub struct PossibleMevTriggers {
    pub is_private:        bool,
    pub coinbase_transfer: bool,
    pub high_priority_fee: bool,
}

self_convert_redefined!(PossibleMevTriggers);

impl fmt::Display for PossibleMevTriggers {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "        {}", "Triggers:".cyan())?;
        if self.is_private {
            writeln!(f, "            - {}", "Private".cyan())?;
        }
        if self.coinbase_transfer {
            writeln!(f, "            - {}", "Coinbase Transfer".cyan())?;
        }
        if self.high_priority_fee {
            writeln!(f, "            - {}", "High Priority Fee".cyan())?;
        }

        Ok(())
    }
}

impl PossibleMevTriggers {
    pub fn was_triggered(&self) -> bool {
        self.coinbase_transfer || self.high_priority_fee
    }
}

impl Serialize for MevBlock {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut ser_struct = serializer.serialize_struct("MevBlock", 33)?;

        ser_struct.serialize_field("block_hash", &format!("{:?}", self.block_hash))?;
        ser_struct.serialize_field("block_number", &self.block_number)?;

        ser_struct.serialize_field("mev_count.mev_count", &vec![self.mev_count.bundle_count])?;
        ser_struct.serialize_field(
            "mev_count.sandwich_count",
            &vec![self.mev_count.sandwich_count.unwrap_or_default()],
        )?;
        ser_struct.serialize_field(
            "mev_count.cex_dex_trade_count",
            &vec![self.mev_count.cex_dex_trade_count.unwrap_or_default()],
        )?;
        ser_struct.serialize_field(
            "mev_count.cex_dex_quote_count",
            &vec![self.mev_count.cex_dex_quote_count.unwrap_or_default()],
        )?;
        ser_struct.serialize_field(
            "mev_count.cex_dex_rfq_count",
            &vec![self.mev_count.cex_dex_rfq_count.unwrap_or_default()],
        )?;
        ser_struct.serialize_field(
            "mev_count.jit_count",
            &vec![self.mev_count.jit_count.unwrap_or_default()],
        )?;
        ser_struct.serialize_field(
            "mev_count.jit_sandwich_count",
            &vec![self.mev_count.jit_sandwich_count.unwrap_or_default()],
        )?;
        ser_struct.serialize_field(
            "mev_count.atomic_backrun_count",
            &vec![self.mev_count.atomic_backrun_count.unwrap_or_default()],
        )?;
        ser_struct.serialize_field(
            "mev_count.liquidation_count",
            &vec![self.mev_count.liquidation_count.unwrap_or_default()],
        )?;

        ser_struct.serialize_field("eth_price", &self.eth_price)?;
        ser_struct.serialize_field("total_gas_used", &self.total_gas_used)?;
        ser_struct.serialize_field("total_priority_fee", &self.total_priority_fee)?;
        ser_struct.serialize_field("total_bribe", &self.total_bribe)?;
        ser_struct.serialize_field("total_mev_bribe", &self.total_mev_bribe)?;
        ser_struct
            .serialize_field("total_mev_priority_fee_paid", &self.total_mev_priority_fee_paid)?;
        ser_struct.serialize_field("builder_address", &format!("{:?}", self.builder_address))?;
        ser_struct.serialize_field("builder_name", &self.builder_name)?;
        ser_struct.serialize_field("builder_eth_profit", &self.builder_eth_profit)?;
        ser_struct.serialize_field("builder_profit_usd", &self.builder_profit_usd)?;
        ser_struct.serialize_field("builder_mev_profit_usd", &self.builder_mev_profit_usd)?;

        ser_struct.serialize_field("builder_searcher_bribes", &self.builder_searcher_bribes)?;
        ser_struct
            .serialize_field("builder_searcher_bribes_usd", &self.builder_searcher_bribes_usd)?;
        ser_struct
            .serialize_field("builder_sponsorship_amount", &self.builder_sponsorship_amount)?;
        ser_struct.serialize_field("ultrasound_bid_adjusted", &self.ultrasound_bid_adjusted)?;

        ser_struct.serialize_field(
            "proposer_fee_recipient",
            &self
                .proposer_fee_recipient
                .map(|addr| format!("{:?}", addr)),
        )?;
        ser_struct.serialize_field("proposer_mev_reward", &self.proposer_mev_reward)?;
        ser_struct.serialize_field("proposer_profit_usd", &self.proposer_profit_usd)?;
        ser_struct.serialize_field("total_mev_profit_usd", &self.total_mev_profit_usd)?;

        let mut possible_tx_hashes = Vec::new();
        let mut possible_tx_idxes = Vec::new();
        let mut possible_gas_coinbases = Vec::new();
        let mut possible_priority_fees = Vec::new();
        let mut possible_gas_useds = Vec::new();
        let mut possible_effective_gas_prices = Vec::new();
        let mut possible_is_privates = Vec::new();
        let mut possible_trigger_coinbases = Vec::new();
        let mut possible_high_priority_fee = Vec::new();
        self.possible_mev
            .0
            .iter()
            .map(|tx| {
                (
                    format!("{:?}", tx.tx_hash),
                    tx.tx_idx,
                    (
                        tx.gas_details.coinbase_transfer,
                        tx.gas_details.priority_fee,
                        tx.gas_details.gas_used,
                        tx.gas_details.effective_gas_price,
                    ),
                    (
                        tx.triggers.is_private,
                        tx.triggers.coinbase_transfer,
                        tx.triggers.high_priority_fee,
                    ),
                )
            })
            .for_each(
                |(
                    hash,
                    idx,
                    (gas_coinbase, priority_fee, gas_used, effective_gas_price),
                    (is_private, trigger_coinbase, high_priority_fee),
                )| {
                    possible_tx_hashes.push(hash);
                    possible_tx_idxes.push(idx);
                    possible_gas_coinbases.push(gas_coinbase);
                    possible_priority_fees.push(priority_fee);
                    possible_gas_useds.push(gas_used);
                    possible_effective_gas_prices.push(effective_gas_price);
                    possible_is_privates.push(is_private);
                    possible_trigger_coinbases.push(trigger_coinbase);
                    possible_high_priority_fee.push(high_priority_fee);
                },
            );

        ser_struct.serialize_field("possible_mev.tx_hash", &possible_tx_hashes)?;
        ser_struct.serialize_field("possible_mev.tx_idx", &possible_tx_idxes)?;
        ser_struct.serialize_field(
            "possible_mev.gas_details.coinbase_transfer",
            &possible_gas_coinbases,
        )?;
        ser_struct
            .serialize_field("possible_mev.gas_details.priority_fee", &possible_priority_fees)?;
        ser_struct.serialize_field("possible_mev.gas_details.gas_used", &possible_gas_useds)?;
        ser_struct.serialize_field(
            "possible_mev.gas_details.effective_gas_price",
            &possible_effective_gas_prices,
        )?;
        ser_struct.serialize_field("possible_mev.triggers.is_private", &possible_is_privates)?;
        ser_struct.serialize_field(
            "possible_mev.triggers.coinbase_transfer",
            &possible_trigger_coinbases,
        )?;
        ser_struct.serialize_field(
            "possible_mev.triggers.high_priority_fee",
            &possible_high_priority_fee,
        )?;

        ser_struct.end()
    }
}

impl DbRow for MevBlock {
    const COLUMN_NAMES: &'static [&'static str] = &[
        "block_hash",
        "block_number",
        "mev_count.mev_count",
        "mev_count.sandwich_count",
        "mev_count.cex_dex_quote_count",
        "mev_count.cex_dex_trade_count",
        "mev_count.cex_dex_rfq_count",
        "mev_count.jit_count",
        "mev_count.jit_sandwich_count",
        "mev_count.atomic_backrun_count",
        "mev_count.liquidation_count",
        "eth_price",
        "total_gas_used",
        "total_priority_fee",
        "total_bribe",
        "total_mev_bribe",
        "total_mev_priority_fee_paid",
        "builder_address",
        "builder_name",
        "builder_eth_profit",
        "builder_profit_usd",
        "builder_mev_profit_usd",
        "builder_searcher_bribes",
        "builder_searcher_bribes_usd",
        "builder_sponsorship_amount",
        "ultrasound_bid_adjusted",
        "proposer_fee_recipient",
        "proposer_mev_reward",
        "proposer_profit_usd",
        "total_mev_profit_usd",
        "possible_mev.tx_hash",
        "possible_mev.tx_idx",
        "possible_mev.gas_details.coinbase_transfer",
        "possible_mev.gas_details.priority_fee",
        "possible_mev.gas_details.gas_used",
        "possible_mev.gas_details.effective_gas_price",
        "possible_mev.triggers.is_private",
        "possible_mev.triggers.coinbase_transfer",
        "possible_mev.triggers.high_priority_fee",
    ];
}
