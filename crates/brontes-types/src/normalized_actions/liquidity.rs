use std::fmt::{self, Debug};

use alloy_primitives::TxHash;
use colored::Colorize;
use malachite::Rational;
use redefined::Redefined;
use reth_primitives::Address;
use rkyv::{Archive, Deserialize as rDeserialize, Serialize as rSerialize};
use serde::{Deserialize, Serialize};
use sorella_db_databases::{
    clickhouse,
    clickhouse::{fixed_string::FixedString, Row},
};

use crate::{
    db::{
        redefined_types::{malachite::RationalRedefined, primitives::AddressRedefined},
        token_info::{TokenInfoWithAddress, TokenInfoWithAddressRedefined},
    },
    Protocol, ToFloatNearest,
};
#[derive(Debug, Default, Serialize, Clone, Row, PartialEq, Eq, Deserialize, Redefined)]
#[redefined_attr(derive(Debug, PartialEq, Clone, Serialize, rSerialize, rDeserialize, Archive))]
pub struct NormalizedMint {
    #[redefined(same_fields)]
    pub protocol:    Protocol,
    pub trace_index: u64,
    pub from:        Address,
    pub to:          Address,
    pub recipient:   Address,
    pub token:       Vec<TokenInfoWithAddress>,
    pub amount:      Vec<Rational>,
}

#[derive(Debug, Default, Serialize, Clone, Row, PartialEq, Eq, Deserialize, Redefined)]
#[redefined_attr(derive(Debug, PartialEq, Clone, Serialize, rSerialize, rDeserialize, Archive))]
pub struct NormalizedBurn {
    #[redefined(same_fields)]
    pub protocol:    Protocol,
    pub trace_index: u64,
    pub from:        Address,
    pub to:          Address,
    pub recipient:   Address,
    pub token:       Vec<TokenInfoWithAddress>,
    pub amount:      Vec<Rational>,
}

#[derive(Debug, Default, Serialize, Clone, Row, PartialEq, Eq, Deserialize, Redefined)]
#[redefined_attr(derive(Debug, PartialEq, Clone, Serialize, rSerialize, rDeserialize, Archive))]
pub struct NormalizedCollect {
    #[redefined(same_fields)]
    pub protocol:    Protocol,
    pub trace_index: u64,
    pub to:          Address,
    pub from:        Address,
    pub recipient:   Address,
    pub token:       Vec<TokenInfoWithAddress>,
    pub amount:      Vec<Rational>,
}

#[derive(Default)]
pub struct ClickhouseVecNormalizedMintOrBurn {
    pub trace_index: Vec<u64>,
    pub from:        Vec<FixedString>,
    pub to:          Vec<FixedString>,
    pub recipient:   Vec<FixedString>,
    pub tokens:      Vec<Vec<FixedString>>,
    pub amounts:     Vec<Vec<[u8; 32]>>,
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

impl From<Vec<NormalizedMint>> for ClickhouseVecNormalizedMintOrBurn {
    fn from(_value: Vec<NormalizedMint>) -> Self {
        todo!("joe");
        // ClickhouseVecNormalizedMintOrBurn {
        //     trace_index: value.iter().map(|val| val.trace_index).collect(),
        //     from:        value
        //         .iter()
        //         .map(|val| format!("{:?}", val.from).into())
        //         .collect(),
        //     to:          value
        //         .iter()
        //         .map(|val| format!("{:?}", val.to).into())
        //         .collect(),
        //     recipient:   value
        //         .iter()
        //         .map(|val| format!("{:?}", val.recipient).into())
        //         .collect(),
        //
        //     tokens:  value
        //         .iter()
        //         .map(|val| {
        //             val.token
        //                 .iter()
        //                 .map(|t| format!("{:?}", t).into())
        //                 .collect_vec()
        //         })
        //         .collect(),
        //     amounts: value
        //         .iter()
        //         .map(|val| val.amount.iter().map(|amt|
        // amt.to_le_bytes()).collect_vec())         .collect(),
        // }
    }
}

impl From<Vec<NormalizedBurn>> for ClickhouseVecNormalizedMintOrBurn {
    fn from(_value: Vec<NormalizedBurn>) -> Self {
        todo!("joe");
        // ClickhouseVecNormalizedMintOrBurn {
        //     trace_index: value.iter().map(|val| val.trace_index).collect(),
        //     from:        value
        //         .iter()
        //         .map(|val| format!("{:?}", val.from).into())
        //         .collect(),
        //     to:          value
        //         .iter()
        //         .map(|val| format!("{:?}", val.to).into())
        //         .collect(),
        //     recipient:   value
        //         .iter()
        //         .map(|val| format!("{:?}", val.recipient).into())
        //         .collect(),
        //
        //     tokens:  value
        //         .iter()
        //         .map(|val| {
        //             val.token
        //                 .iter()
        //                 .map(|t| format!("{:?}", t).into())
        //                 .collect_vec()
        //         })
        //         .collect(),
        //     amounts: value
        //         .iter()
        //         .map(|val| val.amount.iter().map(|amt|
        // amt.to_le_bytes()).collect_vec())         .collect(),
        // }
    }
}

#[derive(Default)]
pub struct ClickhouseVecNormalizedMintOrBurnWithTxHash {
    pub tx_hash:     Vec<FixedString>,
    pub trace_index: Vec<u64>,
    pub from:        Vec<FixedString>,
    pub to:          Vec<FixedString>,
    pub recipient:   Vec<FixedString>,
    pub tokens:      Vec<Vec<FixedString>>,
    pub amounts:     Vec<Vec<[u8; 32]>>,
}

// (tx_hashes, mints)
impl From<(Vec<TxHash>, Vec<Option<Vec<NormalizedMint>>>)>
    for ClickhouseVecNormalizedMintOrBurnWithTxHash
{
    fn from(value: (Vec<TxHash>, Vec<Option<Vec<NormalizedMint>>>)) -> Self {
        let mut this = ClickhouseVecNormalizedMintOrBurnWithTxHash::default();
        value
            .0
            .into_iter()
            .enumerate()
            .filter_map(|(idx, tx_hash)| {
                if let Some(mints) = &value.1[idx] {
                    Some((tx_hash, mints.clone()))
                } else {
                    None
                }
            })
            .map(|(tx_hash, mint)| {
                let tx_hashes_repeated: Vec<FixedString> = vec![tx_hash]
                    .repeat(mint.len())
                    .into_iter()
                    .map(|t| format!("{:?}", t).into())
                    .collect();
                let mint_db: ClickhouseVecNormalizedMintOrBurn = mint.into();
                (tx_hashes_repeated, mint_db)
            })
            .for_each(|(tx_hashes_repeated, db_mint_with_tx)| {
                this.tx_hash.extend(tx_hashes_repeated);
                this.trace_index.extend(db_mint_with_tx.trace_index);
                this.from.extend(db_mint_with_tx.from);
                this.to.extend(db_mint_with_tx.to);
                this.recipient.extend(db_mint_with_tx.recipient);
                this.tokens.extend(db_mint_with_tx.tokens);
                this.amounts.extend(db_mint_with_tx.amounts);
            });

        this
    }
}

// (tx_hashes, burns)
impl From<(Vec<TxHash>, Vec<Option<Vec<NormalizedBurn>>>)>
    for ClickhouseVecNormalizedMintOrBurnWithTxHash
{
    fn from(value: (Vec<TxHash>, Vec<Option<Vec<NormalizedBurn>>>)) -> Self {
        let mut this = ClickhouseVecNormalizedMintOrBurnWithTxHash::default();
        value
            .0
            .into_iter()
            .enumerate()
            .filter_map(|(idx, tx_hash)| {
                if let Some(burns) = &value.1[idx] {
                    Some((tx_hash, burns.clone()))
                } else {
                    None
                }
            })
            .map(|(tx_hash, burn)| {
                let tx_hashes_repeated: Vec<FixedString> = vec![tx_hash]
                    .repeat(burn.len())
                    .into_iter()
                    .map(|t| format!("{:?}", t).into())
                    .collect();
                let burn_db: ClickhouseVecNormalizedMintOrBurn = burn.into();
                (tx_hashes_repeated, burn_db)
            })
            .for_each(|(tx_hashes_repeated, db_burn_with_tx)| {
                this.tx_hash.extend(tx_hashes_repeated);
                this.trace_index.extend(db_burn_with_tx.trace_index);
                this.from.extend(db_burn_with_tx.from);
                this.to.extend(db_burn_with_tx.to);
                this.recipient.extend(db_burn_with_tx.recipient);
                this.tokens.extend(db_burn_with_tx.tokens);
                this.amounts.extend(db_burn_with_tx.amounts);
            });

        this
    }
}
