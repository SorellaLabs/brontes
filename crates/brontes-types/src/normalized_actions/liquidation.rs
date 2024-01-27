use std::fmt::Debug;

use reth_primitives::{Address, U256};
use serde::{Deserialize, Serialize};
use sorella_db_databases::{
    clickhouse,
    clickhouse::{fixed_string::FixedString, Row},
};

pub use super::{Actions, NormalizedSwap};
#[derive(Debug, Serialize, Clone, Row, PartialEq, Eq, Deserialize)]
pub struct NormalizedLiquidation {
    pub trace_index:           u64,
    pub pool:                  Address,
    pub liquidator:            Address,
    pub debtor:                Address,
    pub collateral_asset:      Address,
    pub debt_asset:            Address,
    pub covered_debt:          U256,
    pub liquidated_collateral: U256,
}

impl NormalizedLiquidation {
    pub fn finish_classification(&mut self, actions: Vec<(u64, Actions)>) -> Vec<u64> {
        actions
            .into_iter()
            .find_map(|(index, action)| {
                if let Actions::Transfer(transfer) = action {
                    // because aave has the option to return the Atoken or regular,
                    // we can't filter by collateral filter. This might be an issue...
                    // tbd tho
                    if transfer.to == self.liquidator {
                        self.liquidated_collateral = transfer.amount;
                        return Some(index)
                    }
                }

                None
            })
            .map(|e| vec![e])
            .unwrap_or_default()
    }
}

pub struct ClickhouseVecNormalizedLiquidation {
    pub trace_index:           Vec<u64>,
    pub pool:                  Vec<FixedString>,
    pub liquidator:            Vec<FixedString>,
    pub debtor:                Vec<FixedString>,
    pub collateral_asset:      Vec<FixedString>,
    pub debt_asset:            Vec<FixedString>,
    pub covered_debt:          Vec<[u8; 32]>,
    pub liquidated_collateral: Vec<[u8; 32]>,
}

impl From<Vec<NormalizedLiquidation>> for ClickhouseVecNormalizedLiquidation {
    fn from(value: Vec<NormalizedLiquidation>) -> Self {
        ClickhouseVecNormalizedLiquidation {
            trace_index: value.iter().map(|val| val.trace_index).collect(),
            pool:        value
                .iter()
                .map(|val| format!("{:?}", val.pool).into())
                .collect(),
            liquidator:  value
                .iter()
                .map(|val| format!("{:?}", val.liquidator).into())
                .collect(),
            debtor:      value
                .iter()
                .map(|val| format!("{:?}", val.debtor).into())
                .collect(),

            collateral_asset:      value
                .iter()
                .map(|val| format!("{:?}", val.collateral_asset).into())
                .collect(),
            debt_asset:            value
                .iter()
                .map(|val| format!("{:?}", val.debt_asset).into())
                .collect(),
            covered_debt:          value
                .iter()
                .map(|val| val.covered_debt.to_le_bytes())
                .collect(),
            liquidated_collateral: value
                .iter()
                .map(|val| val.liquidated_collateral.to_le_bytes())
                .collect(),
        }
    }
}
