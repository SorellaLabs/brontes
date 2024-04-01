use std::{
    cmp::{max, min},
    fmt::Display,
};

use alloy_primitives::{wrap_fixed_bytes, FixedBytes};
use clickhouse::Row;
use itertools::Itertools;
use malachite::{
    num::{
        basic::traits::One,
        conversion::{string::options::ToSciOptions, traits::ToSci},
    },
    Rational,
};
use redefined::Redefined;
use reth_db::DatabaseError;
use rkyv::{Archive, Deserialize as rDeserialize, Serialize as rSerialize};
use serde::{Deserialize, Serialize};
use tracing::debug;

use crate::{
    constants::{ETH_ADDRESS, WETH_ADDRESS},
    db::{clickhouse_serde::dex::dex_quote, redefined_types::malachite::RationalRedefined},
    implement_table_value_codecs_with_zc,
    pair::{Pair, PairRedefined},
    FastHashMap,
};

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize, Redefined)]
#[redefined_attr(derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Serialize,
    rDeserialize,
    rSerialize,
    Archive
))]
pub struct DexPrices {
    pub pre_state:    Rational,
    pub post_state:   Rational,
    /// tells us what varient of pricing for this pool we are looking at
    pub goes_through: Pair,
}

impl Display for DexPrices {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut opt = ToSciOptions::default();
        opt.set_scale(9);
        writeln!(f, "pre state price: {}", self.pre_state.to_sci_with_options(opt))?;
        writeln!(f, "post state price: {}", self.post_state.to_sci_with_options(opt))?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum PriceAt {
    Before,
    After,
    Lowest,
    Highest,
    Average,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum BlockPrice {
    Highest,
    Lowest,
    Average,
}

impl DexPrices {
    pub fn get_price(self, post: PriceAt) -> Rational {
        match post {
            PriceAt::After => self.post_state,
            PriceAt::Before => self.pre_state,
            PriceAt::Lowest => min(self.pre_state, self.post_state),
            PriceAt::Highest => max(self.pre_state, self.post_state),
            PriceAt::Average => (self.pre_state + self.post_state) / Rational::from(2),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Row, Eq, Deserialize, Serialize)]
pub struct DexQuotes(pub Vec<Option<FastHashMap<Pair, DexPrices>>>);

impl DexQuotes {
    /// checks for price at the given tx index. if it isn't found, will look for
    /// the price at all previous indexes in the block
    pub fn price_at_or_before(&self, mut pair: Pair, mut tx: usize) -> Option<DexPrices> {
        if pair.0 == ETH_ADDRESS {
            pair.0 = WETH_ADDRESS;
        }
        if pair.1 == ETH_ADDRESS {
            pair.1 = WETH_ADDRESS;
        }
        let s_idx = tx;

        if pair.0 == pair.1 {
            return Some(DexPrices {
                pre_state:    Rational::ONE,
                post_state:   Rational::ONE,
                goes_through: Pair::default(),
            })
        }

        loop {
            if let Some(price) = self.get_price(pair, tx) {
                return Some(price.clone())
            }
            if tx == 0 {
                break
            }

            tx -= 1;
        }

        debug!(?pair, before=?s_idx, "no price for pair");

        None
    }

    pub fn price_for_block(&self, mut pair: Pair, price_at: BlockPrice) -> Option<Rational> {
        if pair.0 == ETH_ADDRESS {
            pair.0 = WETH_ADDRESS;
        }
        if pair.1 == ETH_ADDRESS {
            pair.1 = WETH_ADDRESS;
        }

        match price_at {
            BlockPrice::Lowest => self
                .0
                .iter()
                .filter_map(|f| f.as_ref())
                .filter_map(|p| {
                    p.get(&pair)
                        .map(|prices| prices.clone().get_price(PriceAt::Lowest))
                })
                .min(),
            BlockPrice::Highest => self
                .0
                .iter()
                .filter_map(|f| f.as_ref())
                .filter_map(|p| {
                    p.get(&pair)
                        .map(|prices| prices.clone().get_price(PriceAt::Highest))
                })
                .max(),
            BlockPrice::Average => {
                let entires = self
                    .0
                    .iter()
                    .filter_map(|f| f.as_ref())
                    .filter_map(|p| {
                        p.get(&pair)
                            .map(|prices| prices.clone().get_price(PriceAt::Average))
                    })
                    .collect_vec();

                if entires.is_empty() {
                    return None
                }

                let len = entires.len();
                Some(entires.into_iter().sum::<Rational>() / Rational::from(len))
            }
        }
    }

    pub fn has_quote(&self, pair: &Pair, tx: usize) -> bool {
        self.0
            .get(tx)
            .and_then(|i| i.as_ref().map(|i| i.get(pair).is_some()))
            .unwrap_or(false)
    }

    fn get_price(&self, mut pair: Pair, tx: usize) -> Option<&DexPrices> {
        if pair.0 == ETH_ADDRESS {
            pair.0 = WETH_ADDRESS;
        }
        if pair.1 == ETH_ADDRESS {
            pair.1 = WETH_ADDRESS;
        }
        self.0.get(tx)?.as_ref()?.get(&pair)
    }
}

impl Display for DexQuotes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (i, val) in self.0.iter().enumerate() {
            if let Some(val) = val.as_ref() {
                for (pair, am) in val {
                    writeln!(f, "----Price at tx_index: {i}, pair {:?}-----\n {}", pair, am)?;
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DexQuote(pub FastHashMap<Pair, DexPrices>);

impl From<DexQuoteWithIndex> for DexQuote {
    fn from(value: DexQuoteWithIndex) -> Self {
        Self(value.quote.into_iter().collect())
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, Redefined)]
#[redefined_attr(derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Serialize,
    rDeserialize,
    rSerialize,
    Archive
))]
pub struct DexQuoteWithIndex {
    pub tx_idx: u16,
    pub quote:  Vec<(Pair, DexPrices)>,
}

impl From<DexQuote> for Vec<(Pair, DexPrices)> {
    fn from(val: DexQuote) -> Self {
        val.0.into_iter().collect()
    }
}

implement_table_value_codecs_with_zc!(DexQuoteWithIndexRedefined);

wrap_fixed_bytes!(
    extra_derives: [],
    pub struct DexKey<10>;
);

impl reth_db::table::Encode for DexKey {
    type Encoded = [u8; 10];

    fn encode(self) -> Self::Encoded {
        self.0 .0
    }
}

impl reth_db::table::Decode for DexKey {
    fn decode<B: AsRef<[u8]>>(value: B) -> Result<Self, DatabaseError> {
        Ok(DexKey::from_slice(value.as_ref()))
    }
}

pub fn decompose_key(key: DexKey) -> (u64, u16) {
    let block = FixedBytes::<8>::from_slice(&key[0..8]);
    let block_number = u64::from_be_bytes(*block);

    let tx_idx = FixedBytes::<2>::from_slice(&key[8..]);
    let tx_idx = u16::from_be_bytes(*tx_idx);

    (block_number, tx_idx)
}

pub fn make_key(block_number: u64, tx_idx: u16) -> DexKey {
    let block_bytes = FixedBytes::new(block_number.to_be_bytes());
    block_bytes.concat_const(tx_idx.to_be_bytes().into()).into()
}

pub fn make_filter_key_range(block_number: u64) -> (DexKey, DexKey) {
    let base = FixedBytes::new(block_number.to_be_bytes());
    let start_key = base.concat_const([0u8; 2].into());
    let end_key = base.concat_const([u8::MAX; 2].into());

    (start_key.into(), end_key.into())
}

#[derive(Debug, Clone, PartialEq, Row, Eq, Deserialize, Serialize)]
pub struct DexQuotesWithBlockNumber {
    pub block_number: u64,
    pub tx_idx:       u64,
    #[serde(with = "dex_quote")]
    pub quote:        Option<FastHashMap<Pair, DexPrices>>,
}

impl DexQuotesWithBlockNumber {
    pub fn new_with_block(block_number: u64, quotes: DexQuotes) -> Vec<Self> {
        quotes
            .0
            .into_iter()
            .enumerate()
            .map(|(i, quote)| DexQuotesWithBlockNumber { block_number, tx_idx: i as u64, quote })
            .collect_vec()
    }
}
