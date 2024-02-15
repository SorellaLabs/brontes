use std::fmt::{self, Debug};

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

use crate::db::redefined_types::primitives::{AddressRedefined, B256Redefined};
#[allow(unused_imports)]
use crate::{
    display::utils::display_sandwich,
    normalized_actions::{NormalizedBurn, NormalizedLiquidation, NormalizedMint, NormalizedSwap},
    GasDetails,
};

#[serde_as]
#[derive(Debug, Deserialize, PartialEq, Row, Clone, Default, Redefined)]
#[redefined_attr(derive(Debug, PartialEq, Clone, Serialize, rSer, rDeser, Archive))]
pub struct MevBlock {
    pub block_hash: B256,
    pub block_number: u64,
    #[redefined(same_fields)]
    pub mev_count: MevCount,
    pub eth_price: f64,
    pub cumulative_gas_used: u128,
    pub cumulative_priority_fee: u128,
    pub total_bribe: u128,
    pub cumulative_mev_priority_fee_paid: u128,
    pub builder_address: Address,
    pub builder_eth_profit: f64,
    pub builder_profit_usd: f64,
    // Builder MEV profit from their vertically integrated searchers (in USD)
    pub builder_mev_profit_usd: f64,
    pub proposer_fee_recipient: Option<Address>,
    pub proposer_mev_reward: Option<u128>,
    pub proposer_profit_usd: Option<f64>,
    pub cumulative_mev_profit_usd: f64,
    pub possible_mev: PossibleMevCollection,
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

        writeln!(f, "Block Number: {}", self.block_number)?;
        // Mev section
        writeln!(f, "\n{}", "Mev:".bold().red().underline())?;
        writeln!(f, "{}", self.mev_count.to_string().bold())?;
        writeln!(
            f,
            "  - Cumulative MEV Profit (USD): {}",
            format_profit(self.cumulative_mev_profit_usd)
        )?;
        writeln!(f, "  - Mev Gas:")?;
        writeln!(
            f,
            "    - Total Bribe: {:.6} ETH",
            self.total_bribe as f64 * 1e-18
        )?;
        writeln!(
            f,
            "    - Cumulative MEV Priority Fee Paid: {:.6} ETH",
            self.cumulative_mev_priority_fee_paid as f64 * 1e-18
        )?;

        // Builder section
        writeln!(f, "{}", "Builder:".bold().red().underline())?;
        writeln!(f, "  - Builder Address: {:?}", self.builder_address)?;
        let builder_profit_color = if self.builder_eth_profit < 0.0 {
            "red"
        } else {
            "green"
        };
        writeln!(
            f,
            "  - Builder Profit (USD): {}",
            format_profit(self.builder_profit_usd).color(builder_profit_color)
        )?;
        writeln!(
            f,
            "  - Builder ETH Profit: {:.6} ETH",
            format!("{:.6}", self.builder_eth_profit).color(builder_profit_color)
        )?;
        writeln!(
            f,
            "  - Builder MEV Profit: {:.6} ETH",
            format!("{:.6}", self.builder_mev_profit_usd).green()
        )?;

        // Proposer section
        writeln!(f, "{}", "Proposer:".bold().red().underline())?;

        if self.proposer_fee_recipient.is_none()
            || self.proposer_mev_reward.is_none()
            || self.proposer_profit_usd.is_none()
        {
            writeln!(
                f,
                "{}",
                "  - Isn't an MEV boost block".bold().red().underline()
            )?;
        } else {
            writeln!(
                f,
                "  - Proposer Fee Recipient: {:?}",
                self.proposer_fee_recipient.unwrap()
            )?;
            writeln!(
                f,
                "  - Proposer MEV Reward: {:.6} ETH",
                format!(
                    "{:.6}",
                    self.proposer_mev_reward.unwrap() as f64 / 10f64.powf(18.0)
                )
                .green()
            )?;
            writeln!(
                f,
                "  - Proposer Finalized Profit (USD): {}",
                format_profit(self.proposer_profit_usd.unwrap()).green()
            )?;
        }

        writeln!(
            f,
            "\n{}: {}",
            "Missed Mev".bold().red().underline(),
            self.possible_mev
        )?;

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
    pub mev_count: u64,
    pub sandwich_count: Option<u64>,
    pub cex_dex_count: Option<u64>,
    pub jit_count: Option<u64>,
    pub jit_sandwich_count: Option<u64>,
    pub atomic_backrun_count: Option<u64>,
    pub liquidation_count: Option<u64>,
}

self_convert_redefined!(MevCount);

impl fmt::Display for MevCount {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "  - MEV Count: {}", self.mev_count.to_string().bold())?;

        if let Some(count) = self.sandwich_count {
            writeln!(f, "    - Sandwich: {}", count.to_string().bold())?;
        }
        if let Some(count) = self.cex_dex_count {
            writeln!(f, "    - Cex-Dex: {}", count.to_string().bold())?;
        }
        if let Some(count) = self.jit_count {
            writeln!(f, "    - Jit: {}", count.to_string().bold())?;
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
            format!(
                "Found {} possible MEV Transactions that we did not classify",
                self.0.len()
            )
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
        writeln!(
            f,
            "        Paid {} Eth for inclusion",
            eth_paid.to_string().bold().green()
        )?;
        write!(f, "{}", self.triggers)?;
        writeln!(f, "        Etherscan: {}", tx_url.underline())
    }
}

#[serde_as]
#[derive(Debug, Deserialize, PartialEq, Row, Clone, Default, Redefined)]
#[redefined_attr(derive(Debug, PartialEq, Clone, Serialize, rSer, rDeser, Archive))]
pub struct PossibleMev {
    pub tx_hash: B256,
    pub tx_idx: u64,
    #[redefined(same_fields)]
    pub gas_details: GasDetails,
    #[redefined(same_fields)]
    pub triggers: PossibleMevTriggers,
}

#[serde_as]
#[derive(Debug, PartialEq, Deserialize, Row, Clone, Default, Serialize, rSer, rDeser, Archive)]
pub struct PossibleMevTriggers {
    pub is_private: bool,
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
    //TODO: Currently we don't check for private transactions because there are too
    // many of them we might revisit this once we integrate blocknative so we
    // have a more comprehensive coverage
    pub fn was_triggered(&self) -> bool {
        self.coinbase_transfer || self.high_priority_fee
    }
}

impl Serialize for MevBlock {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut ser_struct = serializer.serialize_struct("MevBlock", 15)?;

        ser_struct.serialize_field(
            "block_hash",
            &FixedString::from(format!("{:?}", self.block_hash)),
        )?;
        ser_struct.serialize_field("block_number", &self.block_number)?;
        ser_struct.serialize_field("mev_count", &self.mev_count)?;
        ser_struct.serialize_field("eth_price", &self.eth_price)?;
        ser_struct.serialize_field("cumulative_gas_used", &self.cumulative_gas_used)?;
        ser_struct.serialize_field("cumulative_priority_fee", &self.cumulative_priority_fee)?;
        ser_struct.serialize_field("total_bribe", &self.total_bribe)?;
        ser_struct.serialize_field(
            "cumulative_mev_priority_fee_paid",
            &self.cumulative_mev_priority_fee_paid,
        )?;
        ser_struct.serialize_field(
            "builder_address",
            &FixedString::from(format!("{:?}", self.builder_address)),
        )?;
        ser_struct.serialize_field("builder_eth_profit", &self.builder_eth_profit)?;
        ser_struct.serialize_field("builder_profit_usd", &self.builder_profit_usd)?;

        ser_struct.serialize_field(
            "proposer_fee_recipient",
            &self
                .proposer_fee_recipient
                .map(|addr| FixedString::from(format!("{:?}", addr))),
        )?;
        ser_struct.serialize_field("proposer_mev_reward", &self.proposer_mev_reward)?;
        ser_struct.serialize_field("proposer_profit_usd", &self.proposer_profit_usd)?;
        ser_struct.serialize_field("cumulative_mev_profit_usd", &self.cumulative_mev_profit_usd)?;

        let possible_mev = self
            .possible_mev
            .0
            .iter()
            .map(|tx| format!("{:?}", tx).into())
            .collect::<Vec<FixedString>>();
        ser_struct.serialize_field("possible_mev", &possible_mev)?;

        ser_struct.end()
    }
}
