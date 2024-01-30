use std::{
    fmt,
    fmt::{Debug, Display},
    ops::{Deref, DerefMut},
};

use alloy_primitives::TxHash;
use colored::Colorize;
use itertools::Itertools;
use malachite::Rational;
use reth_primitives::Address;
use serde::{Deserialize, Serialize};
use sorella_db_databases::{
    clickhouse,
    clickhouse::{fixed_string::FixedString, Row},
};

use crate::{db::token_info::TokenInfoWithAddress, Protocol};

#[derive(Debug, Default, Serialize, Clone, Row, PartialEq, Eq, Deserialize)]
pub struct NormalizedSwapWithFee {
    pub swap:       NormalizedSwap,
    pub fee_token:  TokenInfoWithAddress,
    pub fee_amount: Rational,
}

impl Deref for NormalizedSwapWithFee {
    type Target = NormalizedSwap;

    fn deref(&self) -> &Self::Target {
        &self.swap
    }
}
impl DerefMut for NormalizedSwapWithFee {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.swap
    }
}

#[derive(Debug, Default, Serialize, Clone, Row, PartialEq, Eq, Deserialize)]
pub struct NormalizedSwap {
    pub protocol:    Protocol,
    pub trace_index: u64,
    pub from:        Address,
    pub recipient:   Address,
    // If pool address is zero, then this is a p2p / CoW style swap, possibly within a batch
    pub pool:        Address,
    pub token_in:    TokenInfoWithAddress,
    pub token_out:   TokenInfoWithAddress,
    pub amount_in:   Rational,
    pub amount_out:  Rational,
}

impl NormalizedSwap {
    /// Calculates the rate for a given DEX swap

    pub fn swap_rate(&self) -> Rational {
        // Choose the calculation method based on your standard representation
        &self.amount_in / &self.amount_out
    }
}

impl Display for NormalizedSwap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "   -{}: {} of {} for {} of {} on {}",
            "Swapped".bold(),
            self.amount_in.to_string(),
            self.token_in.to_string(),
            self.amount_out.to_string(),
            self.token_out.to_string(),
            self.pool.to_string()
        )
    }
}

pub struct ClickhouseVecNormalizedSwap {
    pub trace_index: Vec<u64>,
    pub from:        Vec<FixedString>,
    pub recipient:   Vec<FixedString>,
    pub pool:        Vec<FixedString>,
    pub token_in:    Vec<FixedString>,
    pub token_out:   Vec<FixedString>,
    pub amount_in:   Vec<[u8; 32]>,
    pub amount_out:  Vec<[u8; 32]>,
}

impl From<Vec<NormalizedSwap>> for ClickhouseVecNormalizedSwap {
    fn from(_value: Vec<NormalizedSwap>) -> Self {
        todo!("Joe");
        // ClickhouseVecNormalizedSwap {
        //     trace_index: value.iter().map(|val| val.trace_index).collect(),
        //     from:        value
        //         .iter()
        //         .map(|val| format!("{:?}", val.from).into())
        //         .collect(),
        //     recipient:   value
        //         .iter()
        //         .map(|val| format!("{:?}", val.recipient).into())
        //         .collect(),
        //     pool:        value
        //         .iter()
        //         .map(|val| format!("{:?}", val.pool).into())
        //         .collect(),
        //     token_in:    value
        //         .iter()
        //         .map(|val| format!("{:?}", val.token_in).into())
        //         .collect(),
        //     token_out:   value
        //         .iter()
        //         .map(|val| format!("{:?}", val.token_out).into())
        //         .collect(),
        //     amount_in:   value
        //         .iter()
        //         .map(|val| val.amount_in.to_le_bytes())
        //         .collect(),
        //     amount_out:  value
        //         .iter()
        //         .map(|val| val.amount_out.to_le_bytes())
        //         .collect(),
        // }
    }
}

#[derive(Default)]
pub struct ClickhouseDoubleVecNormalizedSwap {
    pub tx_hash:     Vec<FixedString>, /* clickhouse requires nested fields to have the same
                                        * number of rows */
    pub trace_index: Vec<u64>,
    pub from:        Vec<FixedString>,
    pub recipient:   Vec<FixedString>,
    pub pool:        Vec<FixedString>,
    pub token_in:    Vec<FixedString>,
    pub token_out:   Vec<FixedString>,
    pub amount_in:   Vec<[u8; 32]>,
    pub amount_out:  Vec<[u8; 32]>,
}

impl From<(Vec<TxHash>, Vec<Vec<NormalizedSwap>>)> for ClickhouseDoubleVecNormalizedSwap {
    fn from(value: (Vec<TxHash>, Vec<Vec<NormalizedSwap>>)) -> Self {
        let swaps: Vec<(FixedString, ClickhouseVecNormalizedSwap, usize)> = value
            .0
            .into_iter()
            .zip(value.1.into_iter())
            .map(|(tx, swaps)| {
                let num_swaps = swaps.len();
                (format!("{:?}", tx).into(), swaps.into(), num_swaps)
            })
            .collect::<Vec<_>>();

        let mut this = ClickhouseDoubleVecNormalizedSwap::default();

        swaps.into_iter().for_each(|(tx, inner_swaps, num_swaps)| {
            let tx_repeated = (0..num_swaps)
                .into_iter()
                .map(|_| tx.clone())
                .collect::<Vec<FixedString>>();

            if tx_repeated.len() != num_swaps {
                panic!(
                    "The repetitions of tx hash must be equal to the number of swaps when \
                     serializing for clickhouse"
                )
            }

            this.tx_hash.extend(tx_repeated);
            this.trace_index.extend(inner_swaps.trace_index);
            this.from.extend(inner_swaps.from);
            this.recipient.extend(inner_swaps.recipient);
            this.pool.extend(inner_swaps.pool);
            this.token_in.extend(inner_swaps.token_in);
            this.token_out.extend(inner_swaps.token_out);
            this.amount_in.extend(inner_swaps.amount_in);
            this.amount_out.extend(inner_swaps.amount_out);
        });

        this
    }
}

/// i.e. Sandwich: From <victim_tx_hashes, victim_swaps)
impl From<(Vec<Vec<TxHash>>, Vec<Vec<NormalizedSwap>>)> for ClickhouseDoubleVecNormalizedSwap {
    fn from(value: (Vec<Vec<TxHash>>, Vec<Vec<NormalizedSwap>>)) -> Self {
        let tx_hashes = value.0.into_iter().flatten().collect_vec();
        let swaps = value.1;

        (tx_hashes, swaps).into()
    }
}
