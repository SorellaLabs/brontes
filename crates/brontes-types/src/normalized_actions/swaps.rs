use std::{
    fmt,
    fmt::{Debug, Display},
    ops::{Deref, DerefMut},
};

use alloy_primitives::{TxHash, U256};
use colored::Colorize;
use itertools::Itertools;
use malachite::{num::basic::traits::Zero, Rational};
use redefined::Redefined;
use reth_primitives::Address;
use rkyv::{Archive, Deserialize as rDeserialize, Serialize as rSerialize};
use serde::{Deserialize, Serialize};
use sorella_db_databases::{
    clickhouse,
    clickhouse::{fixed_string::FixedString, Row},
};

use super::Actions;
use crate::{
    db::{
        redefined_types::{malachite::*, primitives::*},
        token_info::{TokenInfoWithAddress, TokenInfoWithAddressRedefined},
    },
    mev::StatArbDetails,
    Protocol, ToFloatNearest,
};

#[derive(Debug, Default, Serialize, Deserialize, Clone, Row, PartialEq, Eq)]
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

#[derive(Debug, Default, Serialize, Deserialize, Clone, Row, PartialEq, Eq, Redefined)]
#[redefined_attr(derive(Debug, PartialEq, Clone, Serialize, rSerialize, rDeserialize, Archive))]
pub struct NormalizedSwap {
    #[redefined(same_fields)]
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
    /// Calculates the exchange rate for a given DEX swap
    pub fn swap_rate(&self) -> Rational {
        if self.amount_out == Rational::ZERO {
            return Rational::ZERO
        }

        &self.amount_in / &self.amount_out
    }

    pub fn to_action(&self) -> Actions {
        Actions::Swap(self.clone())
    }
}

impl Display for NormalizedSwap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let amount_in = format!("{:.4}",self.amount_in.clone().to_float()).red();         
        let amount_out = format!("{:.4}", self.amount_out.clone().to_float()).green();         
        let token_in_symbol = self.token_in.symbol.clone();         
        let token_out_symbol = self.token_out.symbol.clone();         
        let protocol: colored::ColoredString = self.protocol.to_string().bold();
        write!(
            f,
            "Swap {} {} to {} {} via {}",
            amount_in,
            token_in_symbol,
            amount_out,
            token_out_symbol,
            protocol
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

#[derive(Default)]
pub struct ClickhouseStatArbDetails {
    pub cex_exchange:     String,
    pub cex_price:        ([u8; 32], [u8; 32]),
    pub dex_exchange:     String,
    pub dex_price:        ([u8; 32], [u8; 32]),
    pub pnl_maker_profit: ([u8; 32], [u8; 32]),
    pub pnl_taker_profit: ([u8; 32], [u8; 32]),
}

impl From<StatArbDetails> for ClickhouseStatArbDetails {
    fn from(value: StatArbDetails) -> Self {
        Self {
            cex_exchange:     format!("{:?}", value.cex_exchange),
            cex_price:        rational_to_u256_bytes(value.cex_price),
            dex_exchange:     value.dex_exchange.to_string(),
            dex_price:        rational_to_u256_bytes(value.dex_price),
            pnl_maker_profit: rational_to_u256_bytes(value.pnl_pre_gas.maker_profit),
            pnl_taker_profit: rational_to_u256_bytes(value.pnl_pre_gas.taker_profit),
        }
    }
}

#[derive(Default)]
pub struct ClickhouseVecStatArbDetails {
    pub cex_exchange:     Vec<String>,
    pub cex_price:        Vec<([u8; 32], [u8; 32])>,
    pub dex_exchange:     Vec<String>,
    pub dex_price:        Vec<([u8; 32], [u8; 32])>,
    pub pnl_maker_profit: Vec<([u8; 32], [u8; 32])>,
    pub pnl_taker_profit: Vec<([u8; 32], [u8; 32])>,
}

impl From<Vec<StatArbDetails>> for ClickhouseVecStatArbDetails {
    fn from(value: Vec<StatArbDetails>) -> Self {
        let mut this = Self::default();

        value.into_iter().for_each(|exch| {
            let val: ClickhouseStatArbDetails = exch.into();
            this.cex_exchange.push(val.cex_exchange);
            this.cex_price.push(val.cex_price);
            this.dex_exchange.push(val.dex_exchange);
            this.dex_price.push(val.dex_price);
            this.pnl_maker_profit.push(val.pnl_maker_profit);
            this.pnl_taker_profit.push(val.pnl_taker_profit);
        });

        this
    }
}

fn rational_to_u256_bytes(value: Rational) -> ([u8; 32], [u8; 32]) {
    let num = U256::from_limbs_slice(&value.numerator_ref().to_limbs_asc());
    let denom = U256::from_limbs_slice(&value.denominator_ref().to_limbs_asc());

    (num.to_le_bytes(), denom.to_le_bytes())
}
