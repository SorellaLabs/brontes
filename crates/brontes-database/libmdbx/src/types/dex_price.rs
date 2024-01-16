use std::collections::HashMap;

use alloy_primitives::TxHash;
use alloy_rlp::{Decodable, Encodable};
use brontes_types::{extra_processing::Pair, impl_compress_decompress_for_encoded_decoded};
use bytes::BufMut;
use malachite::{Natural, Rational};
use reth_db::table::Table;
use serde::{Deserialize, Serialize};
use sorella_db_databases::{clickhouse, Row};

use crate::{tables::DexPrice, LibmdbxData};

pub fn make_key(block_number: u64, tx_idx: u16) -> TxHash {
    let mut bytes = [0u8; 8].to_vec();
    let block_number = block_number.to_be_bytes();
    bytes = [bytes, block_number.to_vec()].concat();
    bytes = [bytes, [0; 14].to_vec()].concat();
    let tx_idx = tx_idx.to_be_bytes();
    bytes = [bytes, tx_idx.to_vec()].concat();
    let key: TxHash = TxHash::from_slice(&bytes);
    key
}

pub fn make_filter_key_range(block_number: u64) -> (TxHash, TxHash) {
    let mut f_bytes = [0u8; 8].to_vec();
    let mut s_bytes = [0u8; 8].to_vec();

    let block_number = block_number.to_be_bytes();
    f_bytes = [f_bytes, block_number.to_vec()].concat();
    s_bytes = [s_bytes, block_number.to_vec()].concat();

    f_bytes = [f_bytes, [0; 16].to_vec()].concat();
    s_bytes = [s_bytes, [u8::MAX; 16].to_vec()].concat();

    let f_key: TxHash = TxHash::from_slice(&f_bytes);
    let s_key: TxHash = TxHash::from_slice(&s_bytes);

    (f_key, s_key)
}

#[derive(Debug, Clone, Row, PartialEq, Eq, Serialize, Deserialize)]
pub struct DexPriceData {
    pub block_number: u64,
    pub tx_idx:       u16,
    pub quote:        DexQuote,
}

impl LibmdbxData<DexPrice> for DexPriceData {
    fn into_key_val(&self) -> (<DexPrice as Table>::Key, <DexPrice as Table>::Value) {
        (
            make_key(self.block_number, self.tx_idx),
            DexQuoteWithIndex { tx_idx: self.tx_idx, quote: self.quote.clone().into() },
        )
    }
}

/*
impl LibmdbxDupData<DexPrice> for DexPriceData {
    fn into_key_subkey_val(
        &self,
    ) -> (<DexPrice as Table>::Key, <DexPrice as DupSort>::SubKey, <DexPrice as Table>::Value) {
        (
            self.block_number,
            self.tx_idx,
            DexQuoteWithIndex { tx_idx: self.tx_idx, quote: self.quote.clone().into() },
        )
    }
}
*/
#[derive(Debug, Clone, Row, PartialEq, Eq, Serialize, Deserialize)]
pub struct DexQuote(pub HashMap<Pair, Rational>);

impl From<DexQuoteWithIndex> for DexQuote {
    fn from(value: DexQuoteWithIndex) -> Self {
        Self(value.quote.into_iter().collect())
    }
}

impl From<DexQuote> for Vec<(Pair, Rational)> {
    fn from(val: DexQuote) -> Self {
        val.0.into_iter().collect()
    }
}

#[derive(Debug, Clone, Row, PartialEq, Eq, Serialize, Deserialize)]
pub struct DexQuoteWithIndex {
    pub tx_idx: u16,
    pub quote:  Vec<(Pair, Rational)>,
}

impl Encodable for DexQuoteWithIndex {
    fn encode(&self, out: &mut dyn BufMut) {
        Encodable::encode(&self.tx_idx, out);
        let (keys, vals): (Vec<_>, Vec<_>) = self.quote.clone().into_iter().unzip();

        let (nums, denoms): (Vec<_>, Vec<_>) = vals
            .into_iter()
            .map(|val| {
                let (n, d) = val.to_numerator_and_denominator();
                (n.to_limbs_asc(), d.to_limbs_asc())
            })
            .unzip();

        keys.encode(out);
        nums.encode(out);
        denoms.encode(out);
    }
}

impl Decodable for DexQuoteWithIndex {
    fn decode(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        let tx_idx = u16::decode(buf)?;

        let keys = Vec::decode(buf)?;
        let nums: Vec<Vec<u64>> = Vec::decode(buf)?;
        let denoms: Vec<Vec<u64>> = Vec::decode(buf)?;

        let prices = nums.into_iter().zip(denoms).map(|(num, denom)| {
            Rational::from_naturals(Natural::from_limbs_asc(&num), Natural::from_limbs_asc(&denom))
        });

        let map = keys.into_iter().zip(prices).collect::<Vec<_>>();

        Ok(Self { tx_idx, quote: map })
    }
}

/*
impl Compact for DexQuoteWithIndex {
    fn to_compact<B>(self, buf: &mut B) -> usize
    where
        B: bytes::BufMut + AsMut<[u8]>,
    {
        buf.put_u16(self.tx_idx);
        Encodable::encode(&self.quote, buf);
        //to_compact() + 2
    }

    fn from_compact(buf: &[u8], len: usize) -> (Self, &[u8]) {
        let tx_idx = u16::from_be_bytes(&buf[..2]);
        let (quote, out) = Vec::from_compact(&buf[2..], len - 2);
        (Self { tx_idx, quote }, out)
    }
}
*/
impl_compress_decompress_for_encoded_decoded!(DexQuoteWithIndex);

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, str::FromStr};

    use alloy_primitives::Address;
    use brontes_database::clickhouse::Clickhouse;
    use brontes_pricing::types::{PoolKey, PoolKeyWithDirection, PoolKeysForPair};

    use super::*;

    fn init_clickhouse() -> Clickhouse {
        dotenv::dotenv().ok();

        Clickhouse::default()
    }

    #[tokio::test]
    async fn test_insert_dex_price_clickhouse() {
        let clickhouse = init_clickhouse();
        let table = "brontes.dex_price_mapping";

        let data = vec![
            DexPriceData {
                block_number: 2,
                tx_idx:       9,
                quote:        DexQuote(Default::default()),
            },
            DexPriceData {
                block_number: 11,
                tx_idx:       9,
                quote:        DexQuote(Default::default()),
            },
            DexPriceData {
                block_number: 10,
                tx_idx:       9,
                quote:        DexQuote(Default::default()),
            },
            DexPriceData {
                block_number: 10,
                tx_idx:       10,
                quote:        DexQuote({
                    let mut map = HashMap::new();
                    map.insert(
                        Pair(
                            Address::from_str("0x0000000000000000000000000000000000000000")
                                .unwrap(),
                            Address::from_str("0x00000000a000000000000a0000000000000a0000")
                                .unwrap(),
                        ),
                        Default::default(),
                    );
                    map.insert(
                        Pair(
                            Address::from_str("0x0000000000000000000000000000000000000000")
                                .unwrap(),
                            Address::from_str("0x00000000a000000000000a0000000000000a0000")
                                .unwrap(),
                        ),
                        vec![PoolKeysForPair(vec![
                            PoolKeyWithDirection::default(),
                            PoolKeyWithDirection {
                                key:  PoolKey {
                                    pool:         Default::default(),
                                    run:          9182,
                                    batch:        102,
                                    update_nonce: 12,
                                },
                                base: Default::default(),
                            },
                        ])],
                    );
                    map
                }),
            },
            DexPriceData {
                block_number: 10,
                tx_idx:       11,
                quote:        DexQuote({
                    let mut map = HashMap::new();
                    map.insert(
                        Pair(
                            Address::from_str("0x2000000000000000000000000000000000000000")
                                .unwrap(),
                            Address::from_str("0x10000000a000000000000a0000000000000a0000")
                                .unwrap(),
                        ),
                        Default::default(),
                    );
                    map.insert(
                        Pair(
                            Address::from_str("0x0000000000000011110000000000000000000000")
                                .unwrap(),
                            Address::from_str("0xef000000a000002200000a0000000000000a0000")
                                .unwrap(),
                        ),
                        vec![PoolKeysForPair(vec![
                            PoolKeyWithDirection::default(),
                            PoolKeyWithDirection {
                                key:  PoolKey {
                                    pool:         Default::default(),
                                    run:          9182,
                                    batch:        102,
                                    update_nonce: 12,
                                },
                                base: Default::default(),
                            },
                        ])],
                    );
                    map
                }),
            },
        ];

        clickhouse.inner().insert_many(data, table).await.unwrap();
    }

    #[test]
    fn test_make_key() {
        let block_number = 18000000;
        let tx_idx = 49;

        let expected =
            TxHash::from_str("0x0000000000000000000000000112A88000000000000000000000000000000031")
                .unwrap();
        let calculated = make_key(block_number, tx_idx);
        println!("CALCULATED: {:?}", calculated);

        assert_eq!(calculated, expected);
    }
}
