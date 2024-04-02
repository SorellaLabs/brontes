use std::{
    fmt,
    fmt::{Debug, Display},
    ops::{Deref, DerefMut},
};

use alloy_primitives::{TxHash, U256};
use clickhouse::Row;
use colored::Colorize;
use itertools::Itertools;
use malachite::{num::basic::traits::Zero, Rational};
use redefined::Redefined;
use reth_primitives::Address;
use rkyv::{Archive, Deserialize as rDeserialize, Serialize as rSerialize};
use serde::{Deserialize, Serialize};

use super::{
    accounting::{apply_delta, AddressDeltas, TokenAccounting},
    Actions,
};
use crate::{
    db::{
        redefined_types::{malachite::*, primitives::*},
        token_info::{TokenInfoWithAddress, TokenInfoWithAddressRedefined},
    },
    mev::StatArbDetails,
    rational_to_clickhouse_tuple, Protocol, ToFloatNearest,
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
    /// For batch swaps (e.g. UniswapX, CowSwap), the pool address is the
    /// address of the settlement contract
    pub pool:        Address,
    pub token_in:    TokenInfoWithAddress,
    pub token_out:   TokenInfoWithAddress,
    pub amount_in:   Rational,
    pub amount_out:  Rational,
    pub msg_value:   U256,
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
        let amount_in = format!("{:.4}", self.amount_in.clone().to_float()).red();
        let amount_out = format!("{:.4}", self.amount_out.clone().to_float()).green();
        let token_in_symbol = self.token_in.symbol.clone();
        let token_out_symbol = self.token_out.symbol.clone();
        let protocol: colored::ColoredString = self.protocol.to_string().bold();
        write!(
            f,
            "Swap {} {} to {} {} via {}",
            amount_in, token_in_symbol, amount_out, token_out_symbol, protocol
        )
    }
}

impl TokenAccounting for NormalizedSwap {
    /// Note that we skip the pool deltas accounting to focus solely on the
    /// swapper & recipients delta. We might want to change this in the
    /// future.
    fn apply_token_deltas(&self, delta_map: &mut AddressDeltas) {
        let amount_in = self.amount_in.clone();
        let amount_out = self.amount_out.clone();

        apply_delta(self.from, self.token_in.address, -amount_in.clone(), delta_map);
        apply_delta(self.recipient, self.token_out.address, amount_out, delta_map);
    }
}

pub struct ClickhouseVecNormalizedSwap {
    pub trace_index: Vec<u64>,
    pub from:        Vec<String>,
    pub recipient:   Vec<String>,
    pub pool:        Vec<String>,
    pub token_in:    Vec<String>,
    pub token_out:   Vec<String>,
    pub amount_in:   Vec<([u8; 32], [u8; 32])>,
    pub amount_out:  Vec<([u8; 32], [u8; 32])>,
}

impl From<Vec<NormalizedSwap>> for ClickhouseVecNormalizedSwap {
    fn from(value: Vec<NormalizedSwap>) -> Self {
        ClickhouseVecNormalizedSwap {
            trace_index: value.iter().map(|val| val.trace_index).collect(),
            from:        value.iter().map(|val| format!("{:?}", val.from)).collect(),
            recipient:   value
                .iter()
                .map(|val| format!("{:?}", val.recipient))
                .collect(),
            pool:        value.iter().map(|val| format!("{:?}", val.pool)).collect(),
            token_in:    value
                .iter()
                .map(|val| format!("{:?}", val.token_in))
                .collect(),
            token_out:   value
                .iter()
                .map(|val| format!("{:?}", val.token_out))
                .collect(),
            amount_in:   value
                .iter()
                .map(|val| rational_to_clickhouse_tuple(&val.amount_in))
                .collect(),
            amount_out:  value
                .iter()
                .map(|val| rational_to_clickhouse_tuple(&val.amount_out))
                .collect(),
        }
    }
}

#[derive(Default)]
pub struct ClickhouseDoubleVecNormalizedSwap {
    pub tx_hash:     Vec<String>,
    pub trace_index: Vec<u64>,
    pub from:        Vec<String>,
    pub recipient:   Vec<String>,
    pub pool:        Vec<String>,
    pub token_in:    Vec<String>,
    pub token_out:   Vec<String>,
    pub amount_in:   Vec<([u8; 32], [u8; 32])>,
    pub amount_out:  Vec<([u8; 32], [u8; 32])>,
}

impl From<(Vec<TxHash>, Vec<Vec<NormalizedSwap>>)> for ClickhouseDoubleVecNormalizedSwap {
    fn from(value: (Vec<TxHash>, Vec<Vec<NormalizedSwap>>)) -> Self {
        let swaps: Vec<(String, ClickhouseVecNormalizedSwap, usize)> = value
            .0
            .into_iter()
            .zip(value.1)
            .map(|(tx, swaps)| {
                let num_swaps = swaps.len();
                (format!("{:?}", tx), swaps.into(), num_swaps)
            })
            .collect::<Vec<_>>();

        let mut this = ClickhouseDoubleVecNormalizedSwap::default();

        swaps.into_iter().for_each(|(tx, inner_swaps, num_swaps)| {
            let tx_repeated = (0..num_swaps).map(|_| tx.clone()).collect::<Vec<String>>();

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
            cex_exchange:     value.cex_exchange.to_string(),
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
    pub cex_exchanges:    Vec<String>,
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
            this.cex_exchanges.push(val.cex_exchange);
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
