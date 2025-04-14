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
use alloy_primitives::Address;
use rkyv::{Archive, Deserialize as rDeserialize, Serialize as rSerialize};
use serde::{Deserialize, Serialize};

use super::{
    accounting::{apply_delta, AddressDeltas, TokenAccounting},
    Action,
};
use crate::{
    db::{
        redefined_types::{malachite::*, primitives::*},
        token_info::{TokenInfoWithAddress, TokenInfoWithAddressRedefined},
    },
    mev::ArbDetails,
    rational_to_u256_fraction, Protocol, ToFloatNearest,
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

#[derive(Debug, Default, Serialize, Deserialize, Clone, Row, PartialEq, Eq, Redefined, Hash)]
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

    pub fn to_action(&self) -> Action {
        Action::Swap(self.clone())
    }

    pub fn token_in_symbol(&self) -> &str {
        self.token_in.symbol.as_str()
    }

    pub fn token_out_symbol(&self) -> &str {
        self.token_out.symbol.as_str()
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
    pub token_in:    Vec<(String, String)>,
    pub token_out:   Vec<(String, String)>,
    pub amount_in:   Vec<([u8; 32], [u8; 32])>,
    pub amount_out:  Vec<([u8; 32], [u8; 32])>,
}

impl TryFrom<Vec<NormalizedSwap>> for ClickhouseVecNormalizedSwap {
    type Error = eyre::Report;

    fn try_from(value: Vec<NormalizedSwap>) -> eyre::Result<Self> {
        Ok(ClickhouseVecNormalizedSwap {
            trace_index: value.iter().map(|val| val.trace_index).collect(),
            from:        value.iter().map(|val| format!("{:?}", val.from)).collect(),
            recipient:   value
                .iter()
                .map(|val| format!("{:?}", val.recipient))
                .collect(),
            pool:        value.iter().map(|val| format!("{:?}", val.pool)).collect(),
            token_in:    value
                .iter()
                .map(|val| val.token_in.clickhouse_fmt())
                .collect(),
            token_out:   value
                .iter()
                .map(|val| val.token_out.clickhouse_fmt())
                .collect(),
            amount_in:   value
                .iter()
                .map(|val| rational_to_u256_fraction(&val.amount_in))
                .collect::<eyre::Result<Vec<_>>>()?,
            amount_out:  value
                .iter()
                .map(|val| rational_to_u256_fraction(&val.amount_out))
                .collect::<eyre::Result<Vec<_>>>()?,
        })
    }
}

#[derive(Default)]
pub struct ClickhouseDoubleVecNormalizedSwap {
    pub tx_hash:     Vec<String>,
    pub trace_index: Vec<u64>,
    pub from:        Vec<String>,
    pub recipient:   Vec<String>,
    pub pool:        Vec<String>,
    pub token_in:    Vec<(String, String)>,
    pub token_out:   Vec<(String, String)>,
    pub amount_in:   Vec<([u8; 32], [u8; 32])>,
    pub amount_out:  Vec<([u8; 32], [u8; 32])>,
}

impl TryFrom<(Vec<TxHash>, Vec<Vec<NormalizedSwap>>)> for ClickhouseDoubleVecNormalizedSwap {
    type Error = eyre::Report;

    fn try_from(value: (Vec<TxHash>, Vec<Vec<NormalizedSwap>>)) -> eyre::Result<Self> {
        let swaps: Vec<(String, ClickhouseVecNormalizedSwap, usize)> = value
            .0
            .into_iter()
            .zip(value.1)
            .map(|(tx, swaps)| {
                let num_swaps = swaps.len();
                swaps
                    .try_into()
                    .map(|s| (format!("{:?}", tx), s, num_swaps))
            })
            .collect::<eyre::Result<Vec<_>>>()?;

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

        Ok(this)
    }
}

/// i.e. Sandwich: From <victim_tx_hashes, victim_swaps)
impl TryFrom<(Vec<Vec<TxHash>>, Vec<Vec<NormalizedSwap>>)> for ClickhouseDoubleVecNormalizedSwap {
    type Error = eyre::Report;

    fn try_from(value: (Vec<Vec<TxHash>>, Vec<Vec<NormalizedSwap>>)) -> eyre::Result<Self> {
        let tx_hashes = value.0.into_iter().flatten().collect_vec();
        let swaps = value.1;

        (tx_hashes, swaps).try_into()
    }
}

#[derive(Default)]
pub struct ClickhouseArbDetails {
    pub cex_exchange:  String,
    pub cex_price:     ([u8; 32], [u8; 32]),
    pub dex_exchange:  String,
    pub dex_price:     ([u8; 32], [u8; 32]),
    pub pnl_maker_mid: ([u8; 32], [u8; 32]),
    pub pnl_taker_mid: ([u8; 32], [u8; 32]),
    pub pnl_maker_ask: ([u8; 32], [u8; 32]),
    pub pnl_taker_ask: ([u8; 32], [u8; 32]),
}

impl TryFrom<ArbDetails> for ClickhouseArbDetails {
    type Error = eyre::Report;

    fn try_from(_value: ArbDetails) -> eyre::Result<Self> {
        todo!();
        /*

        Ok(Self {
            cex_exchange:  value.cex_exchange.to_string(),
            cex_price:     rational_to_u256_fraction(&value.cex_price)?,
            dex_exchange:  value.dex_exchange.to_string(),
            dex_price:     rational_to_u256_fraction(&value.dex_price)?,
            pnl_maker_mid: rational_to_u256_fraction(&value.pnl_pre_gas.maker_taker_mid.0)?,
            pnl_taker_mid: rational_to_u256_fraction(&value.pnl_pre_gas.maker_taker_mid.1)?,
            pnl_maker_ask: rational_to_u256_fraction(&value.pnl_pre_gas.maker_taker_ask.0)?,
            pnl_taker_ask: rational_to_u256_fraction(&value.pnl_pre_gas.maker_taker_ask.1)?,
        })*/
    }
}

#[derive(Default)]
pub struct ClickhouseVecArbDetails {
    pub cex_exchanges: Vec<String>,
    pub cex_price:     Vec<([u8; 32], [u8; 32])>,
    pub dex_exchange:  Vec<String>,
    pub dex_price:     Vec<([u8; 32], [u8; 32])>,
    pub pnl_maker_mid: Vec<([u8; 32], [u8; 32])>,
    pub pnl_taker_mid: Vec<([u8; 32], [u8; 32])>,
    pub pnl_maker_ask: Vec<([u8; 32], [u8; 32])>,
    pub pnl_taker_ask: Vec<([u8; 32], [u8; 32])>,
}

impl TryFrom<Vec<ArbDetails>> for ClickhouseVecArbDetails {
    type Error = eyre::Report;

    fn try_from(value: Vec<ArbDetails>) -> eyre::Result<Self> {
        let mut this = Self::default();

        value
            .into_iter()
            .map(|exch| exch.try_into())
            .collect::<eyre::Result<Vec<ClickhouseArbDetails>>>()?
            .into_iter()
            .for_each(|val| {
                this.cex_exchanges.push(val.cex_exchange);
                this.cex_price.push(val.cex_price);
                this.dex_exchange.push(val.dex_exchange);
                this.dex_price.push(val.dex_price);
                this.pnl_maker_mid.push(val.pnl_maker_mid);
                this.pnl_taker_mid.push(val.pnl_taker_mid);
                this.pnl_maker_ask.push(val.pnl_maker_ask);
                this.pnl_taker_ask.push(val.pnl_taker_ask);
            });

        Ok(this)
    }
}
