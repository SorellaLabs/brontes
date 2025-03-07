use std::fmt::{self, Debug};

use alloy_primitives::{Address, U256};
use clickhouse::Row;
use colored::Colorize;
use malachite::Rational;
use redefined::Redefined;
use rkyv::{Archive, Deserialize as rDeserialize, Serialize as rSerialize};
use serde::{Deserialize, Serialize};

use super::accounting::{apply_delta, AddressDeltas, TokenAccounting};
pub use super::{Action, NormalizedSwap};
use crate::{
    db::{
        redefined_types::{malachite::RationalRedefined, primitives::*},
        token_info::{TokenInfoWithAddress, TokenInfoWithAddressRedefined},
    },
    rational_to_u256_fraction, Protocol,
};

#[derive(Default, Debug, Serialize, Clone, Row, PartialEq, Eq, Deserialize, Redefined)]
#[redefined_attr(derive(Debug, PartialEq, Clone, Serialize, rSerialize, rDeserialize, Archive))]
pub struct NormalizedLiquidation {
    #[redefined(same_fields)]
    pub protocol:              Protocol,
    pub trace_index:           u64,
    pub pool:                  Address,
    pub liquidator:            Address,
    pub debtor:                Address,
    pub collateral_asset:      TokenInfoWithAddress,
    pub debt_asset:            TokenInfoWithAddress,
    pub covered_debt:          Rational,
    pub liquidated_collateral: Rational,
    pub msg_value:             U256,
}

impl TokenAccounting for NormalizedLiquidation {
    fn apply_token_deltas(&self, delta_map: &mut AddressDeltas) {
        let debt_covered = self.covered_debt.clone();
        // Liquidator sends debt_asset to the pool, effectively swapping the debt asset
        // for the liquidatee's collateral
        apply_delta(self.pool, self.debt_asset.address, debt_covered.clone(), delta_map);
        // the assets don't nessacarly come from the liquidatior. can come directly from
        // a pool liquidator swapped on
        // apply_delta(self.liquidator, self.debt_asset.address, -debt_covered,
        // delta_map);

        // Pool sends collateral to the liquidator
        apply_delta(
            self.pool,
            self.collateral_asset.address,
            -self.liquidated_collateral.clone(),
            delta_map,
        );

        // Liquidator gains collateral asset
        apply_delta(
            self.liquidator,
            self.collateral_asset.address,
            self.liquidated_collateral.clone(),
            delta_map,
        )
    }
}

impl fmt::Display for NormalizedLiquidation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let protocol = self.protocol.to_string().bold();
        let pool_address = format!("{}", self.pool).cyan();
        let liquidator_address = format!("{}", self.liquidator).cyan();
        let debtor_address = format!("{}", self.debtor).cyan();
        let collateral_asset_symbol = self.collateral_asset.inner.symbol.bold();
        let debt_asset_symbol = self.debt_asset.inner.symbol.bold();
        let covered_debt_formatted = format!("{:.4}", self.covered_debt).green();
        let liquidated_collateral_formatted = format!("{:.4}", self.liquidated_collateral).red();

        write!(
            f,
            "Protocol {} - Pool: {}, Liquidator: {}, Debtor: {}, Collateral: {}, Debt: {}, \
             Covered Debt: {}, Liquidated Collateral: {}",
            protocol,
            pool_address,
            liquidator_address,
            debtor_address,
            collateral_asset_symbol,
            debt_asset_symbol,
            covered_debt_formatted,
            liquidated_collateral_formatted
        )
    }
}

impl NormalizedLiquidation {
    pub fn pretty_print(&self, f: &mut fmt::Formatter<'_>, spaces: usize) -> fmt::Result {
        let field_names = [
            "Protocol",
            "Pool",
            "Liquidator",
            "Debtor",
            "Collateral",
            "Debt",
            "Covered Debt",
            "Liquidated Collateral",
        ];
        let max_field_name_length = field_names.iter().map(|name| name.len()).max().unwrap_or(0);
        let indent = " ".repeat(spaces);

        let protocol = self.protocol.to_string().bright_yellow();
        let pool_address = format!("{}", self.pool).bright_yellow();
        let liquidator_address = format!("{}", self.liquidator).bright_yellow();
        let debtor_address = format!("{}", self.debtor).bright_yellow();
        let collateral_asset_symbol = self.collateral_asset.inner.symbol.clone().bright_yellow();
        let debt_asset_symbol = self.debt_asset.inner.symbol.clone().bright_yellow();
        let covered_debt_formatted = format!("{:.4}", self.covered_debt).bright_yellow();
        let liquidated_collateral_formatted =
            format!("{:.4}", self.liquidated_collateral).bright_yellow();

        writeln!(
            f,
            "{indent}{:width$}: {}\n{indent}{:width$}: {}\n{indent}{:width$}: \
             {}\n{indent}{:width$}: {}\n{indent}{:width$}: {}\n{indent}{:width$}: \
             {}\n{indent}{:width$}: {}\n{indent}{:width$}: {}",
            "Protocol",
            protocol,
            "Pool",
            pool_address,
            "Liquidator",
            liquidator_address,
            "Debtor",
            debtor_address,
            "Collateral",
            collateral_asset_symbol,
            "Debt",
            debt_asset_symbol,
            "Covered Debt",
            covered_debt_formatted,
            "Liquidated Collateral",
            liquidated_collateral_formatted,
            indent = indent,
            width = max_field_name_length + spaces + 1
        )?;

        Ok(())
    }
}

pub struct ClickhouseVecNormalizedLiquidation {
    pub trace_index:           Vec<u64>,
    pub pool:                  Vec<String>,
    pub liquidator:            Vec<String>,
    pub debtor:                Vec<String>,
    pub collateral_asset:      Vec<(String, String)>,
    pub debt_asset:            Vec<(String, String)>,
    pub covered_debt:          Vec<([u8; 32], [u8; 32])>,
    pub liquidated_collateral: Vec<([u8; 32], [u8; 32])>,
}

impl TryFrom<Vec<NormalizedLiquidation>> for ClickhouseVecNormalizedLiquidation {
    type Error = eyre::Report;

    fn try_from(value: Vec<NormalizedLiquidation>) -> eyre::Result<Self> {
        Ok(ClickhouseVecNormalizedLiquidation {
            trace_index: value.iter().map(|val| val.trace_index).collect(),
            pool:        value.iter().map(|val| format!("{:?}", val.pool)).collect(),
            liquidator:  value
                .iter()
                .map(|val| format!("{:?}", val.liquidator))
                .collect(),
            debtor:      value
                .iter()
                .map(|val| format!("{:?}", val.debtor))
                .collect(),

            collateral_asset:      value
                .iter()
                .map(|val| val.collateral_asset.clickhouse_fmt())
                .collect(),
            debt_asset:            value
                .iter()
                .map(|val| val.debt_asset.clickhouse_fmt())
                .collect(),
            covered_debt:          value
                .iter()
                .map(|val| rational_to_u256_fraction(&val.covered_debt))
                .collect::<eyre::Result<Vec<_>>>()?,
            liquidated_collateral: value
                .iter()
                .map(|val| rational_to_u256_fraction(&val.liquidated_collateral))
                .collect::<eyre::Result<Vec<_>>>()?,
        })
    }
}
