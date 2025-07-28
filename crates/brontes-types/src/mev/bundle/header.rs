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
        searcher::Fund,
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
    pub block_number: u64,

    pub tx_index:              u64,
    #[serde(with = "txhash")]
    // For a sandwich this is always the first frontrun tx hash
    pub tx_hash: B256,
    #[serde(with = "addresss")]
    pub eoa:                   Address,
    #[serde(with = "option_addresss")]
    pub mev_contract:          Option<Address>,
    #[redefined(same_fields)]
    #[serde(default)]
    pub fund:                  Fund,
    pub profit_usd:            f64,
    // Total tx cost in USD
    pub bribe_usd:             f64,
    #[redefined(same_fields)]
    pub mev_type:              MevType,
    // if we generated this arb without pricing
    pub no_pricing_calculated: bool,
    pub balance_deltas:        Vec<TransactionAccounting>,
    pub timeboosted:           bool,
}

#[serde_as]
#[derive(Debug, Deserialize, Row, PartialEq, Clone, Default, Serialize, Redefined)]
#[redefined_attr(derive(Debug, PartialEq, Clone, Serialize, rSerialize, rDeserialize, Archive))]
pub struct TransactionAccounting {
    pub tx_hash:        B256,
    pub address_deltas: Vec<AddressBalanceDeltas>,
}

impl Display for TransactionAccounting {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{} {}", "Token Deltas for tx:".bold(), self.tx_hash)?;

        self.address_deltas.iter().for_each(|address_deltas| {
            writeln!(f, "{}", address_deltas).expect("Failed to output AddressBalanceDeltas");
        });

        Ok(())
    }
}
#[serde_as]
#[derive(Debug, Deserialize, Row, PartialEq, Clone, Default, Serialize, Redefined)]
#[redefined_attr(derive(Debug, PartialEq, Clone, Serialize, rSerialize, rDeserialize, Archive))]
pub struct AddressBalanceDeltas {
    pub address:      Address,
    pub name:         Option<String>,
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

impl Display for AddressBalanceDeltas {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let header = if let Some(name) = &self.name {
            format!("Address Balance Changes for {}: {}", name.bold(), self.address)
        } else {
            format!("Address Balance Changes for {}", self.address)
        };

        writeln!(f, "{}", header)?;

        for delta in &self.token_deltas {
            let amount_display = if delta.amount >= 0.0 {
                format!("{:+.7}", delta.amount).green()
            } else {
                format!("{:+.7}", delta.amount).red()
            };

            writeln!(
                f,
                "  - {}: {} (USD Value: ${:.2})",
                delta.token.inner.symbol.bold(),
                amount_display,
                delta.usd_value
            )?;
        }

        Ok(())
    }
}

impl Serialize for BundleHeader {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut ser_struct = serializer.serialize_struct("BundleHeader", 12)?;

        ser_struct.serialize_field("block_number", &self.block_number)?;
        ser_struct.serialize_field("tx_index", &self.tx_index)?;
        ser_struct.serialize_field("tx_hash", &format!("{:?}", &self.tx_hash))?;
        ser_struct.serialize_field("eoa", &format!("{:?}", &self.eoa))?;
        ser_struct
            .serialize_field("mev_contract", &self.mev_contract.map(|a| format!("{:?}", a)))?;
        ser_struct.serialize_field("fund", &self.fund)?;
        ser_struct.serialize_field("profit_usd", &self.profit_usd)?;
        ser_struct.serialize_field("bribe_usd", &self.bribe_usd)?;
        ser_struct.serialize_field("mev_type", &self.mev_type)?;
        ser_struct.serialize_field("no_pricing_calculated", &self.no_pricing_calculated)?;

        let balance_deltas_tx_hashes = self
            .balance_deltas
            .iter()
            .flat_map(|b| {
                [b.tx_hash]
                    .repeat(b.address_deltas.len())
                    .into_iter()
                    .map(|val| format!("{:?}", val))
            })
            .collect_vec();
        ser_struct.serialize_field("balance_deltas.tx_hash", &balance_deltas_tx_hashes)?;

        let balance_deltas_addresses = self
            .balance_deltas
            .iter()
            .flat_map(|b| {
                b.address_deltas
                    .iter()
                    .map(|delta| format!("{:?}", delta.address))
            })
            .collect_vec();
        ser_struct.serialize_field("balance_deltas.address", &balance_deltas_addresses)?;

        let balance_deltas_names = self
            .balance_deltas
            .iter()
            .flat_map(|b| b.address_deltas.iter().map(|delta| delta.name.clone()))
            .collect_vec();
        ser_struct.serialize_field("balance_deltas.name", &balance_deltas_names)?;

        let balance_deltas_token_deltas = self
            .balance_deltas
            .iter()
            .flat_map(|b| {
                b.address_deltas.iter().map(|delta| {
                    delta
                        .token_deltas
                        .iter()
                        .map(|token_delta| {
                            (
                                (
                                    format!("{:?}", token_delta.token.address),
                                    token_delta.token.inner.decimals,
                                    token_delta.token.inner.symbol.clone(),
                                ),
                                token_delta.amount,
                                token_delta.usd_value,
                            )
                        })
                        .collect_vec()
                })
            })
            .collect_vec();
        ser_struct.serialize_field("balance_deltas.token_deltas", &balance_deltas_token_deltas)?;
        ser_struct.serialize_field("timeboosted", &self.timeboosted)?;
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
        "fund",
        "profit_usd",
        "bribe_usd",
        "mev_type",
        "no_pricing_calculated",
        "balance_deltas.tx_hash",
        "balance_deltas.address",
        "balance_deltas.name",
        "balance_deltas.token_deltas",
        "timeboosted",
    ];
}
