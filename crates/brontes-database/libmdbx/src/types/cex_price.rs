use std::{collections::HashMap, default::Default, ops::MulAssign, str::FromStr};

use alloy_primitives::Address;
use alloy_rlp::{Decodable, Encodable};
use brontes_database::clickhouse::types::DBTokenPricesDB;
use brontes_types::extra_processing::Pair;
use bytes::BufMut;
use malachite::{
    num::arithmetic::traits::{Floor, ReciprocalAssign},
    platform_64::Limb,
    Natural, Rational,
};
use reth_db::{
    table::{Compress, Decompress},
    DatabaseError,
};
use serde::{Deserialize, Serialize};
use sorella_db_databases::{clickhouse, Row};

use crate::{tables::CexPrice, LibmdbxData};

#[derive(Debug, Clone, Row, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct CexPriceData {
    pub block_num:     u64,
    pub cex_price_map: CexPriceMap,
}

impl LibmdbxData<CexPrice> for CexPriceData {
    fn into_key_val(
        &self,
    ) -> (<CexPrice as reth_db::table::Table>::Key, <CexPrice as reth_db::table::Table>::Value)
    {
        (self.block_num, self.cex_price_map.clone())
    }
}

/// Each pair is entered into the map with the addresses in order by value:
/// Ergo if token0 < token1, then the pair is (token0, token1)
/// So when we query the map we order the addresses in the pair and then query
/// the quote provides us with the actual token0 so we can interpret the price
/// in any direction
#[derive(Debug, Clone, Row, PartialEq, Eq, Serialize)]
pub struct CexPriceMap(pub HashMap<Pair, Vec<CexQuote>>);

impl Default for CexPriceMap {
    fn default() -> Self {
        Self::new()
    }
}

impl CexPriceMap {
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    pub fn wrap(map: HashMap<Pair, CexQuote>) -> Self {
        Self(map.into_iter().map(|(k, v)| (k, vec![v])).collect())
    }

    /// Assumes binance quote, for retro compatibility
    pub fn get_quote(&self, pair: &Pair) -> Option<CexQuote> {
        let ordered_pair = pair.ordered();
        self.0.get(&ordered_pair).and_then(|quotes| {
            quotes.first().map(|quote| {
                if quote.token0 == pair.0 {
                    quote.clone()
                } else {
                    let mut reciprocal_quote = quote.clone();
                    reciprocal_quote.inverse_price(); // Modify the price to its reciprocal
                    reciprocal_quote
                }
            })
        })
    }

    pub fn get_binance_quote(&self, pair: &Pair) -> Option<CexQuote> {
        let ordered_pair = pair.ordered();
        self.0.get(&ordered_pair).and_then(|quotes| {
            quotes.first().map(|quote| {
                if quote.token0 == pair.0 {
                    quote.clone()
                } else {
                    let mut reciprocal_quote = quote.clone();
                    reciprocal_quote.inverse_price(); // Modify the price to its reciprocal
                    reciprocal_quote
                }
            })
        })
    }

    pub fn get_avg_quote(&self, pair: &Pair) -> Option<CexQuote> {
        let ordered_pair = pair.ordered();
        self.0.get(&ordered_pair).and_then(|quotes| {
            if quotes.is_empty() {
                None
            } else {
                let (sum_price, count) = quotes.iter().fold(
                    ((Rational::default(), Rational::default()), 0),
                    |(acc, cnt), q| {
                        let mut quote = q.clone();
                        if quote.token0 != pair.0 {
                            quote.inverse_price();
                        }
                        ((acc.0 + quote.price.0, acc.1 + quote.price.1), cnt + 1)
                    },
                );
                let count = Rational::from(count);
                Some(CexQuote {
                    exchange:  None,
                    timestamp: quotes.last().unwrap().timestamp,
                    price:     (sum_price.0 / count.clone(), sum_price.1 / count),
                    token0:    pair.0,
                })
            }
        })
    }
}

impl From<Vec<DBTokenPricesDB>> for CexPriceMap {
    fn from(value: Vec<DBTokenPricesDB>) -> Self {
        let mut map: HashMap<Pair, Vec<CexQuote>> = HashMap::new();

        for token_info in value {
            let pair = Pair::map_key(
                Address::from_str(&token_info.key.0).unwrap(),
                Address::from_str(&token_info.key.1).unwrap(),
            );

            let quotes: Vec<CexQuote> = token_info
                .val
                .into_iter()
                .map(|exchange_price| {
                    CexQuote {
                        exchange:  Some(exchange_price.exchange),
                        timestamp: exchange_price.val.0,
                        price:     (
                            Rational::try_from(exchange_price.val.1).unwrap(), /* Conversion to
                                                                                * Rational */
                            Rational::try_from(exchange_price.val.2).unwrap(),
                        ),
                        token0:    Address::from_str(&token_info.key.0).unwrap(),
                    }
                })
                .collect();

            map.insert(pair, quotes);
        }

        CexPriceMap(map)
    }
}
impl Encodable for CexPriceMap {
    fn encode(&self, out: &mut dyn BufMut) {
        let val = self.0.clone().into_iter().collect::<Vec<_>>();
        let (pairs, quotes): (Vec<Pair>, Vec<Vec<CexQuote>>) = val.into_iter().unzip();
        pairs.encode(out);
        quotes.encode(out);
    }
}

impl Decodable for CexPriceMap {
    fn decode(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        let mut map = HashMap::new();
        let pairs = Vec::decode(buf)?;
        let quotes = Vec::decode(buf)?;

        pairs.into_iter().zip(quotes).for_each(|(pair, quote)| {
            map.entry(pair).or_insert(quote);
        });

        Ok(Self(map))
    }
}

impl Compress for CexPriceMap {
    type Compressed = Vec<u8>;

    fn compress_to_buf<B: reth_primitives::bytes::BufMut + AsMut<[u8]>>(self, buf: &mut B) {
        let mut encoded = Vec::new();
        self.encode(&mut encoded);
        buf.put_slice(&encoded);
    }
}

impl Decompress for CexPriceMap {
    fn decompress<B: AsRef<[u8]>>(value: B) -> Result<Self, reth_db::DatabaseError> {
        let binding = value.as_ref().to_vec();
        let buf = &mut binding.as_slice();
        CexPriceMap::decode(buf).map_err(|_| DatabaseError::Decode)
    }
}

impl<'de> Deserialize<'de> for CexPriceMap {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let map: Vec<((String, String), Vec<(Option<String>, u64, (f64, f64), String)>)> =
            Deserialize::deserialize(deserializer)?;

        let mut cex_price_map = HashMap::new();
        map.into_iter().for_each(|(pair, meta)| {
            cex_price_map.insert(
                Pair(Address::from_str(&pair.0).unwrap(), Address::from_str(&pair.1).unwrap()),
                meta.into_iter()
                    .map(|(exchange, timestamp, (price0, price1), token0)| CexQuote {
                        exchange,
                        timestamp,
                        price: (
                            Rational::try_from_float_simplest(price0).unwrap(),
                            Rational::try_from_float_simplest(price1).unwrap(),
                        ),
                        token0: Address::from_str(&token0).unwrap(),
                    })
                    .collect::<Vec<_>>(),
            );
        });

        Ok(CexPriceMap(cex_price_map))
    }
}

#[derive(Debug, Clone, Default, Row, Eq, Serialize, Deserialize)]
pub struct CexQuote {
    pub exchange:  Option<String>,
    pub timestamp: u64,
    /// Best Ask & Bid price at p2p timestamp (which is when the block is first
    /// propagated by the relay / proposer)
    pub price:     (Rational, Rational),
    pub token0:    Address,
}

impl CexQuote {
    fn inverse_price(&mut self) {
        self.price.0.reciprocal_assign();
        self.price.1.reciprocal_assign();
    }
}
impl CexQuote {
    pub fn avg(&self) -> Rational {
        (&self.price.0 + &self.price.1) / Rational::from(2)
    }

    pub fn best_ask(&self) -> Rational {
        self.price.0.clone()
    }

    pub fn best_bid(&self) -> Rational {
        self.price.1.clone()
    }
}

impl PartialEq for CexQuote {
    fn eq(&self, other: &Self) -> bool {
        self.timestamp == other.timestamp
            && (self.price.0.clone() * Rational::try_from(1000000000).unwrap()).floor()
                == (other.price.0.clone() * Rational::try_from(1000000000).unwrap()).floor()
            && (self.price.1.clone() * Rational::try_from(1000000000).unwrap()).floor()
                == (other.price.1.clone() * Rational::try_from(1000000000).unwrap()).floor()
    }
}

impl MulAssign for CexQuote {
    fn mul_assign(&mut self, rhs: Self) {
        self.price.0 *= rhs.price.0;
        self.price.1 *= rhs.price.1;
    }
}

impl Encodable for CexQuote {
    fn encode(&self, out: &mut dyn BufMut) {
        Encodable::encode(&self.exchange.clone().unwrap_or_default(), out);
        Encodable::encode(&self.timestamp, out);
        Encodable::encode(&self.price.0.numerator_ref().to_limbs_asc(), out);
        Encodable::encode(&self.price.0.denominator_ref().to_limbs_asc(), out);
        Encodable::encode(&self.price.1.numerator_ref().to_limbs_asc(), out);
        Encodable::encode(&self.price.1.denominator_ref().to_limbs_asc(), out);
        self.token0.encode(out);
    }
}

impl Decodable for CexQuote {
    fn decode(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        let exchange_str = String::decode(buf)?;
        let mut exchange = None;
        if exchange_str.is_empty() {
            exchange = Some(exchange_str);
        }
        let timestamp = u64::decode(buf)?;

        let price0_num = Natural::from_limbs_asc(&Vec::<Limb>::decode(buf)?);
        let price0_denom = Natural::from_limbs_asc(&Vec::<Limb>::decode(buf)?);
        let price0 = Rational::from_naturals(price0_num, price0_denom);

        let price1_num = Natural::from_limbs_asc(&Vec::<Limb>::decode(buf)?);
        let price1_denom = Natural::from_limbs_asc(&Vec::<Limb>::decode(buf)?);
        let price1 = Rational::from_naturals(price1_num, price1_denom);

        let token0 = Address::decode(buf)?;

        Ok(CexQuote { exchange, timestamp, price: (price0, price1), token0 })
    }
}

impl Compress for CexQuote {
    type Compressed = Vec<u8>;

    fn compress_to_buf<B: reth_primitives::bytes::BufMut + AsMut<[u8]>>(self, buf: &mut B) {
        let mut encoded = Vec::new();
        self.encode(&mut encoded);
        buf.put_slice(&encoded);
    }
}

impl Decompress for CexQuote {
    fn decompress<B: AsRef<[u8]>>(value: B) -> Result<Self, reth_db::DatabaseError> {
        let binding = value.as_ref().to_vec();
        let buf = &mut binding.as_slice();
        CexQuote::decode(buf).map_err(|_| DatabaseError::Decode)
    }
}

/*
#[cfg(test)]
mod tests {

    use brontes_database::clickhouse::Clickhouse;

    use super::*;

    fn init_clickhouse() -> Clickhouse {
        dotenv::dotenv().ok();

        Clickhouse::default()
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 5)]
    async fn test_insert_dex_price_clickhouse() {
        let clickhouse = init_clickhouse();
        let _table = "brontes.cex_price_mapping";

        let res = clickhouse
            .inner()
            .query_many::<CexPriceData>(
                "SELECT
        block_number,
        data AS meta
    FROM brontes.cex_price_mapping WHERE block_number >= 16200000 AND block_number < 16300000",
                &(), //table,
            )
            .await;

        assert!(res.is_ok());

        //println!("{:?}", res.unwrap()[0])
    }
}
*/
