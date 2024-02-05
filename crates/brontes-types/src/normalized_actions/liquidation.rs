use std::fmt::Debug;

use malachite::Rational;
use redefined::Redefined;
use reth_primitives::Address;
use rkyv::{Archive, Deserialize as rDeserialize, Serialize as rSerialize};
use serde::{Deserialize, Serialize};
use sorella_db_databases::{
    clickhouse,
    clickhouse::{fixed_string::FixedString, Row},
};

pub use super::{Actions, NormalizedSwap};
use crate::{
    db::{
        redefined_types::{malachite::RationalRedefined, primitives::*},
        token_info::{TokenInfoWithAddress, TokenInfoWithAddressRedefined},
    },
    Protocol,
};

#[derive(Debug, Serialize, Clone, Row, PartialEq, Eq, Deserialize, Redefined)]
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
    fn from(_value: Vec<NormalizedLiquidation>) -> Self {
        todo!("todo");
        // ClickhouseVecNormalizedLiquidation {
        //     trace_index: value.iter().map(|val| val.trace_index).collect(),
        //     pool:        value
        //         .iter()
        //         .map(|val| format!("{:?}", val.pool).into())
        //         .collect(),
        //     liquidator:  value
        //         .iter()
        //         .map(|val| format!("{:?}", val.liquidator).into())
        //         .collect(),
        //     debtor:      value
        //         .iter()
        //         .map(|val| format!("{:?}", val.debtor).into())
        //         .collect(),
        //
        //     collateral_asset:      value
        //         .iter()
        //         .map(|val| format!("{:?}", val.collateral_asset).into())
        //         .collect(),
        //     debt_asset:            value
        //         .iter()
        //         .map(|val| format!("{:?}", val.debt_asset).into())
        //         .collect(),
        //     covered_debt:          value
        //         .iter()
        //         .map(|val| val.covered_debt.to_le_bytes())
        //         .collect(),
        //     liquidated_collateral: value
        //         .iter()
        //         .map(|val| val.liquidated_collateral.to_le_bytes())
        //         .collect(),
        // }
    }
}
