use std::fmt::{self, Debug, Display};

use alloy_primitives::Address;
use clickhouse::{DbRow, Row};
use colored::Colorize;
use itertools::Itertools;
use redefined::Redefined;
use reth_primitives::B256;
use rkyv::{Archive, Deserialize as rDeserialize, Serialize as rSerialize};
use serde::{ser::SerializeStruct, Deserialize, Serialize};
use serde_with::serde_as;

use super::MevType;
use crate::{
    db::{
        redefined_types::primitives::*,
        token_info::{TokenInfoWithAddress, TokenInfoWithAddressRedefined},
    },
    serde_utils::{addresss, option_addresss, txhash},
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
pub struct BundleHeader {
    pub block_number:   u64,
    pub tx_index:       u64,
    #[serde(with = "txhash")]
    // For a sandwich this is always the first frontrun tx hash
    pub tx_hash: B256,
    #[serde(with = "addresss")]
    pub eoa:            Address,
    #[serde(with = "option_addresss")]
    pub mev_contract:   Option<Address>,
    pub profit_usd:     f64,
    pub bribe_usd:      f64,
    #[redefined(same_fields)]
    pub mev_type:       MevType,
    pub balance_deltas: Vec<TransactionAccounting>,
}

#[serde_as]
#[derive(Debug, Deserialize, Row, PartialEq, Clone, Default, Serialize, Redefined)]
#[redefined_attr(derive(Debug, PartialEq, Clone, Serialize, rSerialize, rDeserialize, Archive))]
pub struct TransactionAccounting {
    pub address_deltas: Vec<AddressBalanceDeltas>,
}

#[serde_as]
#[derive(Debug, Deserialize, Row, PartialEq, Clone, Default, Serialize, Redefined)]
#[redefined_attr(derive(Debug, PartialEq, Clone, Serialize, rSerialize, rDeserialize, Archive))]
pub struct AddressBalanceDeltas {
    pub address:      Address,
    pub token_deltas: Vec<TokenBalanceDelta>,
}

#[serde_as]
#[derive(Debug, Deserialize, Row, PartialEq, Clone, Default, Serialize, Redefined)]
#[redefined_attr(derive(Debug, PartialEq, Clone, Serialize, rSerialize, rDeserialize, Archive))]
pub struct TokenBalanceDelta {
    pub token:     TokenInfoWithAddress,
    pub amount:    f64,
    pub usd_value: f64,
}

impl BalanceDeltas {
    pub fn compose(&mut self, to_compose: &BalanceDeltas) {
        for profit in &to_compose.transaction_deltas {
            if let Some(existing_profit) = self
                .transaction_deltas
                .iter_mut()
                .find(|p| p.profit_collector == profit.profit_collector && p.token == profit.token)
            {
                if existing_profit.amount < profit.amount {
                    existing_profit.amount = profit.amount;
                }
            }
        }
    }

    pub fn print_with_labels(
        &self,
        f: &mut fmt::Formatter,
        mev_contract_address: Option<Address>,
        eoa_address: Address,
    ) -> fmt::Result {
        writeln!(f, "\n{}", "Token Deltas:\n".bold().bright_white().underline())?;

        for profit in &self.transaction_deltas {
            let collector_label = match profit.profit_collector {
                _ if Some(profit.profit_collector) == mev_contract_address => "MEV Contract",
                _ if profit.profit_collector == eoa_address => "EOA",
                _ => "Other",
            };

            let amount_display = if profit.amount >= 0.0 {
                format!("{:.7}", profit.amount).green()
            } else {
                format!("{:.7}", profit.amount.abs()).red()
            };

            writeln!(
                f,
                " - {}: {} {}: {} (worth ${:.2})",
                collector_label.bright_white(),
                if profit.amount >= 0.0 { "Gained" } else { "Lost" },
                profit.token.inner.symbol.bold(),
                amount_display,
                profit.usd_value.abs()
            )?;
        }

        Ok(())
    }
}

impl Display for BalanceDeltas {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.print_with_labels(f, None, Address::default())?;

        Ok(())
    }
}

impl Display for BalanceDelta {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let (profit_or_loss, amount_str, usd_value_str) = if self.amount < 0.0 {
            ("lost", self.amount.to_string().red(), format!("$ {}", self.usd_value).red())
        } else {
            ("gained", self.amount.to_string().green(), format!("$ {}", self.usd_value).green())
        };

        writeln!(
            f,
            "Address: {} {} {} {} worth {}",
            self.transaction_deltas,
            profit_or_loss,
            amount_str,
            self.token.symbol.bold(),
            usd_value_str
        )
    }
}

impl Serialize for BundleHeader {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut ser_struct = serializer.serialize_struct("BundleHeader", 10)?;

        ser_struct.serialize_field("block_number", &self.block_number)?;
        ser_struct.serialize_field("tx_index", &self.tx_index)?;
        ser_struct.serialize_field("tx_hash", &format!("{:?}", &self.tx_hash))?;
        ser_struct.serialize_field("eoa", &format!("{:?}", &self.eoa))?;
        ser_struct
            .serialize_field("mev_contract", &self.mev_contract.map(|a| format!("{:?}", a)))?;
        ser_struct.serialize_field("profit_usd", &self.profit_usd)?;

        let profit_collector = self
            .token_profits
            .profits
            .iter()
            .map(|tp| format!("{:?}", tp.profit_collector))
            .collect_vec();
        let token = self
            .token_profits
            .profits
            .iter()
            .map(|tp| {
                (
                    format!("{:?}", tp.token.address),
                    tp.token.decimals,
                    tp.token.inner.symbol.clone(),
                )
            })
            .collect_vec();
        let amount = self
            .token_profits
            .profits
            .iter()
            .map(|tp| tp.amount)
            .collect_vec();
        let usd_value = self
            .token_profits
            .profits
            .iter()
            .map(|tp| tp.usd_value)
            .collect_vec();

        ser_struct.serialize_field("token_profits.profit_collector", &profit_collector)?;
        ser_struct.serialize_field("token_profits.token", &token)?;
        ser_struct.serialize_field("token_profits.amount", &amount)?;
        ser_struct.serialize_field("token_profits.usd_value", &usd_value)?;

        ser_struct.serialize_field("bribe_usd", &self.bribe_usd)?;
        ser_struct.serialize_field("mev_type", &self.mev_type)?;

        ser_struct.end()
    }
}

impl DbRow for BundleHeader {
    const COLUMN_NAMES: &'static [&'static str] = &[
        "block_number",
        "tx_index",
        "tx_hash",
        "eoa",
        "mev_contract",
        "profit_usd",
        "token_profits.profit_collector",
        "token_profits.token",
        "token_profits.amount",
        "token_profits.usd_value",
        "bribe_usd",
        "mev_type",
    ];
}
