use std::fmt::{self, Debug};

use alloy_primitives::{Address, TxHash};
use clickhouse::Row;
use colored::Colorize;
use itertools::Itertools;
use malachite::Rational;
use redefined::Redefined;
use rkyv::{Archive, Deserialize as rDeserialize, Serialize as rSerialize};
use serde::{Deserialize, Serialize};

use super::accounting::{apply_delta, AddressDeltas, TokenAccounting};
use crate::{
    db::{
        redefined_types::{malachite::RationalRedefined, primitives::AddressRedefined},
        token_info::{TokenInfoWithAddress, TokenInfoWithAddressRedefined},
    },
    rational_to_u256_fraction, Protocol, ToFloatNearest,
};
#[derive(Debug, Default, Serialize, Clone, Row, PartialEq, Eq, Deserialize, Redefined)]
#[redefined_attr(derive(Debug, PartialEq, Clone, Serialize, rSerialize, rDeserialize, Archive))]
pub struct NormalizedMint {
    #[redefined(same_fields)]
    pub protocol:    Protocol,
    pub trace_index: u64,
    pub from:        Address,
    pub recipient:   Address,
    pub pool:        Address,
    pub token:       Vec<TokenInfoWithAddress>,
    pub amount:      Vec<Rational>,
}

impl TokenAccounting for NormalizedMint {
    fn apply_token_deltas(&self, delta_map: &mut AddressDeltas) {
        self.amount.iter().enumerate().for_each(|(index, amount)| {
            let amount_minted = -amount.clone();
            apply_delta(self.from, self.token[index].address, amount_minted, delta_map);
            apply_delta(self.pool, self.token[index].address, amount.clone(), delta_map);
        });
    }
}

#[derive(Debug, Default, Serialize, Clone, Row, PartialEq, Eq, Deserialize, Redefined)]
#[redefined_attr(derive(Debug, PartialEq, Clone, Serialize, rSerialize, rDeserialize, Archive))]
pub struct NormalizedBurn {
    #[redefined(same_fields)]
    pub protocol:    Protocol,
    pub trace_index: u64,
    pub from:        Address,
    pub recipient:   Address,
    pub pool:        Address,
    pub token:       Vec<TokenInfoWithAddress>,
    pub amount:      Vec<Rational>,
}

impl TokenAccounting for NormalizedBurn {
    fn apply_token_deltas(&self, delta_map: &mut AddressDeltas) {
        self.amount.iter().enumerate().for_each(|(index, amount)| {
            let amount_burned = -amount.clone();
            apply_delta(self.pool, self.token[index].address, amount_burned, delta_map);
            apply_delta(self.recipient, self.token[index].address, amount.clone(), delta_map);
        });
    }
}

#[derive(Debug, Default, Serialize, Clone, Row, PartialEq, Eq, Deserialize, Redefined)]
#[redefined_attr(derive(Debug, PartialEq, Clone, Serialize, rSerialize, rDeserialize, Archive))]
pub struct NormalizedCollect {
    #[redefined(same_fields)]
    pub protocol:    Protocol,
    pub trace_index: u64,
    pub from:        Address,
    pub recipient:   Address,
    pub pool:        Address,
    pub token:       Vec<TokenInfoWithAddress>,
    pub amount:      Vec<Rational>,
}

impl TokenAccounting for NormalizedCollect {
    fn apply_token_deltas(&self, delta_map: &mut AddressDeltas) {
        self.amount.iter().enumerate().for_each(|(index, amount)| {
            let amount_collected = -amount.clone();
            apply_delta(self.pool, self.token[index].address, amount_collected, delta_map);
            apply_delta(self.recipient, self.token[index].address, amount.clone(), delta_map);
        });
    }
}

#[derive(Default)]
pub struct ClickhouseVecNormalizedMintOrBurn {
    pub trace_index: Vec<u64>,
    pub from:        Vec<String>,
    pub pool:        Vec<String>,
    pub recipient:   Vec<String>,
    pub tokens:      Vec<Vec<(String, String)>>,
    pub amounts:     Vec<Vec<([u8; 32], [u8; 32])>>,
}

impl fmt::Display for NormalizedMint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let protocol = self.protocol.to_string().bold();
        let mint_info: Vec<_> = self
            .token
            .iter()
            .zip(self.amount.iter())
            .map(|(token, amount)| {
                let token_symbol = token.inner.symbol.bold();
                let amount_formatted = format!("{:.4}", amount.clone().to_float()).green();
                format!("{} {}", amount_formatted, token_symbol)
            })
            .collect();

        write!(f, "Added [{}] Liquidity on {}", mint_info.join(", "), protocol)
    }
}

impl fmt::Display for NormalizedBurn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let protocol = self.protocol.to_string().bold();
        let mint_info: Vec<_> = self
            .token
            .iter()
            .zip(self.amount.iter())
            .map(|(token, amount)| {
                let token_symbol = token.inner.symbol.bold();
                let amount_formatted = format!("{:.4}", amount.clone().to_float()).red();
                format!("{} {}", amount_formatted, token_symbol)
            })
            .collect();

        write!(f, "Removed [{}] Liquidity on {}", mint_info.join(", "), protocol)
    }
}

impl fmt::Display for NormalizedCollect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let protocol = self.protocol.to_string().bold();
        let mint_info: Vec<_> = self
            .token
            .iter()
            .zip(self.amount.iter())
            .map(|(token, amount)| {
                let token_symbol = token.inner.symbol.bold();
                let amount_formatted = format!("{:.4}", amount.clone().to_float()).green();
                format!("{} {}", amount_formatted, token_symbol)
            })
            .collect();

        write!(f, "Collect [{}] Fees on {}", mint_info.join(", "), protocol)
    }
}

impl TryFrom<Vec<NormalizedMint>> for ClickhouseVecNormalizedMintOrBurn {
    type Error = eyre::Report;

    fn try_from(value: Vec<NormalizedMint>) -> eyre::Result<Self> {
        Ok(ClickhouseVecNormalizedMintOrBurn {
            trace_index: value.iter().map(|val| val.trace_index).collect(),
            from:        value.iter().map(|val| format!("{:?}", val.from)).collect(),
            pool:        value.iter().map(|val| format!("{:?}", val.pool)).collect(),
            recipient:   value
                .iter()
                .map(|val| format!("{:?}", val.recipient))
                .collect(),

            tokens:  value
                .iter()
                .map(|val| val.token.iter().map(|t| t.clickhouse_fmt()).collect_vec())
                .collect(),
            amounts: value
                .iter()
                .map(|val| {
                    val.amount
                        .iter()
                        .map(rational_to_u256_fraction)
                        .collect::<eyre::Result<Vec<_>>>()
                })
                .collect::<eyre::Result<Vec<_>>>()?,
        })
    }
}

impl TryFrom<Vec<NormalizedBurn>> for ClickhouseVecNormalizedMintOrBurn {
    type Error = eyre::Report;

    fn try_from(value: Vec<NormalizedBurn>) -> eyre::Result<Self> {
        Ok(ClickhouseVecNormalizedMintOrBurn {
            trace_index: value.iter().map(|val| val.trace_index).collect(),
            from:        value.iter().map(|val| format!("{:?}", val.from)).collect(),
            pool:        value.iter().map(|val| format!("{:?}", val.pool)).collect(),
            recipient:   value
                .iter()
                .map(|val| format!("{:?}", val.recipient))
                .collect(),

            tokens:  value
                .iter()
                .map(|val| val.token.iter().map(|t| t.clickhouse_fmt()).collect_vec())
                .collect(),
            amounts: value
                .iter()
                .map(|val| {
                    val.amount
                        .iter()
                        .map(rational_to_u256_fraction)
                        .collect::<eyre::Result<Vec<_>>>()
                })
                .collect::<eyre::Result<Vec<_>>>()?,
        })
    }
}

#[derive(Default)]
pub struct ClickhouseVecNormalizedMintOrBurnWithTxHash {
    pub tx_hash:     Vec<String>,
    pub trace_index: Vec<u64>,
    pub from:        Vec<String>,
    pub pool:        Vec<String>,
    pub recipient:   Vec<String>,
    pub tokens:      Vec<Vec<(String, String)>>,
    pub amounts:     Vec<Vec<([u8; 32], [u8; 32])>>,
}

// (tx_hashes, mints)
impl TryFrom<(Vec<TxHash>, Vec<Option<Vec<NormalizedMint>>>)>
    for ClickhouseVecNormalizedMintOrBurnWithTxHash
{
    type Error = eyre::Report;

    fn try_from(value: (Vec<TxHash>, Vec<Option<Vec<NormalizedMint>>>)) -> eyre::Result<Self> {
        let mut this = ClickhouseVecNormalizedMintOrBurnWithTxHash::default();
        value
            .0
            .into_iter()
            .enumerate()
            .filter_map(|(idx, tx_hash)| {
                value.1[idx].as_ref().map(|mints| (tx_hash, mints.clone()))
            })
            .map(|(tx_hash, mint)| {
                let tx_hashes_repeated: Vec<String> = [tx_hash]
                    .repeat(mint.len())
                    .into_iter()
                    .map(|t| format!("{:?}", t))
                    .collect();
                mint.try_into()
                    .map(|mint_db: ClickhouseVecNormalizedMintOrBurn| (tx_hashes_repeated, mint_db))
            })
            .collect::<eyre::Result<Vec<_>>>()?
            .into_iter()
            .for_each(|(tx_hashes_repeated, db_mint_with_tx)| {
                this.tx_hash.extend(tx_hashes_repeated);
                this.trace_index.extend(db_mint_with_tx.trace_index);
                this.from.extend(db_mint_with_tx.from);
                this.pool.extend(db_mint_with_tx.pool);
                this.recipient.extend(db_mint_with_tx.recipient);
                this.tokens.extend(db_mint_with_tx.tokens);
                this.amounts.extend(db_mint_with_tx.amounts);
            });

        Ok(this)
    }
}

// (tx_hashes, burns)
impl TryFrom<(Vec<TxHash>, Vec<Option<Vec<NormalizedBurn>>>)>
    for ClickhouseVecNormalizedMintOrBurnWithTxHash
{
    type Error = eyre::Report;

    fn try_from(value: (Vec<TxHash>, Vec<Option<Vec<NormalizedBurn>>>)) -> eyre::Result<Self> {
        let mut this = ClickhouseVecNormalizedMintOrBurnWithTxHash::default();
        value
            .0
            .into_iter()
            .enumerate()
            .filter_map(|(idx, tx_hash)| value.1[idx].as_ref().map(|burn| (tx_hash, burn.clone())))
            .map(|(tx_hash, burn)| {
                let tx_hashes_repeated: Vec<String> = [tx_hash]
                    .repeat(burn.len())
                    .into_iter()
                    .map(|t| format!("{:?}", t))
                    .collect();
                burn.try_into()
                    .map(|burn_db: ClickhouseVecNormalizedMintOrBurn| (tx_hashes_repeated, burn_db))
            })
            .collect::<eyre::Result<Vec<_>>>()?
            .into_iter()
            .for_each(|(tx_hashes_repeated, db_burn_with_tx)| {
                this.tx_hash.extend(tx_hashes_repeated);
                this.trace_index.extend(db_burn_with_tx.trace_index);
                this.from.extend(db_burn_with_tx.from);
                this.pool.extend(db_burn_with_tx.pool);
                this.recipient.extend(db_burn_with_tx.recipient);
                this.tokens.extend(db_burn_with_tx.tokens);
                this.amounts.extend(db_burn_with_tx.amounts);
            });

        Ok(this)
    }
}
