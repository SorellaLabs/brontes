use std::{
    cmp::{max, min},
    fmt::Display,
    str::FromStr,
};

use alloy_primitives::{wrap_fixed_bytes, Address, FixedBytes};
use clickhouse::Row;
use itertools::Itertools;
use malachite::{
    num::{
        basic::traits::One,
        conversion::{string::options::ToSciOptions, traits::ToSci},
    },
    Natural, Rational,
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

/// Represents the DEX prices of a token pair before (`pre_state`) and after a
/// transaction (`post_state`)
///
/// The `goes_through` field, indicates the token pair of the pool
/// that generated the action that caused the pricing event.
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
    /// tells us what variant of pricing for this pool we are looking at
    pub goes_through: Pair,
    /// lets us know if this price was generated from a transfer. This allows
    /// us to choose a swap that will have a correct goes through for the given
    /// tx over a transfer which will be less accurate on price
    pub is_transfer:  bool,
}

impl Display for DexPrices {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut opt = ToSciOptions::default();
        opt.set_scale(9);
        writeln!(f, "pre state price: {}", self.pre_state.to_sci_with_options(opt))?;
        writeln!(f, "post state price: {}", self.post_state.to_sci_with_options(opt))?;
        writeln!(f, "goes through: {:?}", self.goes_through)?;
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

/// A collection of dex prices for a given block
///
/// Each index in the vec represents a tx index in the block
///
/// For a given transaction, the value is `None` if it doesn't
/// contain any token transfers
#[derive(Debug, Clone, PartialEq, Row, Eq, Deserialize, Serialize)]
pub struct DexQuotes(pub Vec<Option<FastHashMap<Pair, DexPrices>>>);

impl DexQuotes {
    /// This is done as the require tokens for our testing sets
    /// the index to zero
    #[cfg(feature = "test_pricing")]
    pub fn price_at(&self, mut pair: Pair, mut tx: usize) -> Option<DexPrices> {
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
                is_transfer:  false,
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

        debug!(target: "brontes::missing_pricing_query",?pair, at_or_before=?s_idx, "no price for pair");

        None
    }

    /// checks for price at the given tx index. if it isn't found, will look for
    /// the price at all previous indexes in the block
    #[cfg(not(feature = "test_pricing"))]
    pub fn price_at(&self, mut pair: Pair, tx: usize) -> Option<DexPrices> {
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
                is_transfer:  false,
            })
        }

        if let Some(price) = self.get_price(pair, tx) {
            return Some(price.clone())
        }

        debug!(target: "brontes::missing_pricing_query",?pair, at=?s_idx, "no price for pair");

        None
    }

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
                is_transfer:  false,
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

        debug!(target: "brontes::missing_pricing_query",?pair, at_or_before=?s_idx, "no price for pair");

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

#[derive(Debug, Default, Clone, PartialEq, Eq, serde::Serialize, Redefined)]
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

type DexPriceQuotesVec = (
    u64,
    Vec<((String, String), ((Vec<u64>, Vec<u64>), (Vec<u64>, Vec<u64>), (String, String), bool))>,
);

impl<'de> Deserialize<'de> for DexQuoteWithIndex {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let des: DexPriceQuotesVec = Deserialize::deserialize(deserializer)?;

        if des.1.is_empty() {
            return Ok(DexQuoteWithIndex { tx_idx: des.0 as u16, quote: vec![] })
        }

        let val = des
            .1
            .into_iter()
            .map(|((pair0, pair1), ((pre_num, pre_den), (post_num, post_den), (g0, g1), t))| {
                (
                    Pair(Address::from_str(&pair0).unwrap(), Address::from_str(&pair1).unwrap()),
                    DexPrices {
                        pre_state:    Rational::from_naturals(
                            Natural::from_owned_limbs_asc(pre_num),
                            Natural::from_owned_limbs_asc(pre_den),
                        ),
                        post_state:   Rational::from_naturals(
                            Natural::from_owned_limbs_asc(post_num),
                            Natural::from_owned_limbs_asc(post_den),
                        ),
                        goes_through: Pair(
                            Address::from_str(&g0).unwrap(),
                            Address::from_str(&g1).unwrap(),
                        ),
                        is_transfer:  t,
                    },
                )
            })
            .collect::<Vec<_>>();
        Ok(Self { tx_idx: des.0 as u16, quote: val })
    }
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
