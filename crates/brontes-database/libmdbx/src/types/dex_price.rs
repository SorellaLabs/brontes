use std::{collections::HashMap, default::Default, hash::Hash, ops::MulAssign, str::FromStr};

use alloy_primitives::{hex::FromHexError, Address, TxHash, U256};
use alloy_rlp::{Decodable, Encodable};
use brontes_database::clickhouse::types::DBTokenPricesDB;
use brontes_pricing::types::{DexPrices, PoolKey, PoolKeysForPair};
use brontes_types::{extra_processing::Pair, impl_compress_decompress_for_encoded_decoded};
use bytes::BufMut;
use codecs_derive::derive_arbitrary;
use malachite::{
    num::{
        arithmetic::traits::{Floor, ReciprocalAssign},
        conversion::traits::RoundingFrom,
    },
    platform_64::Limb,
    rounding_modes::RoundingMode,
    Integer, Natural, Rational,
};
use parity_scale_codec::Encode;
use reth_codecs::{main_codec, Compact};
use reth_db::{
    table::{Compress, Decompress, DupSort, Table},
    DatabaseError,
};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use sorella_db_databases::{clickhouse, Row};

use super::{utils::dex_quote, LibmdbxDupData};
use crate::{
    tables::{CexPrice, DexPrice},
    types::utils::pool_tokens,
    LibmdbxData,
};

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

#[derive(Debug, Clone, Row, PartialEq, Eq, Serialize, Deserialize)]
pub struct DexPriceData {
    pub block_number: u64,
    pub tx_idx:       u16,
    #[serde(with = "dex_quote")]
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
pub struct DexQuote(pub HashMap<Pair, Vec<PoolKeysForPair>>);

impl From<DexQuoteWithIndex> for DexQuote {
    fn from(value: DexQuoteWithIndex) -> Self {
        Self(value.quote.into_iter().collect())
    }
}

impl Into<Vec<(Pair, Vec<PoolKeysForPair>)>> for DexQuote {
    fn into(self) -> Vec<(Pair, Vec<PoolKeysForPair>)> {
        self.0.into_iter().collect()
    }
}

#[derive(Debug, Clone, Row, PartialEq, Eq, Serialize, Deserialize)]
pub struct DexQuoteWithIndex {
    pub tx_idx: u16,
    pub quote:  Vec<(Pair, Vec<PoolKeysForPair>)>,
}

impl Encodable for DexQuoteWithIndex {
    fn encode(&self, out: &mut dyn BufMut) {
        Encodable::encode(&self.tx_idx, out);
        let (keys, vals): (Vec<_>, Vec<_>) =
            self.quote.clone().into_iter().map(|(k, v)| (k, v)).unzip();

        keys.encode(out);
        vals.encode(out);
    }
}

impl Decodable for DexQuoteWithIndex {
    fn decode(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        let tx_idx = u16::decode(buf)?;

        let keys = Vec::decode(buf)?;
        let vals = Vec::decode(buf)?;

        let map = keys.into_iter().zip(vals.into_iter()).collect::<Vec<_>>();

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
    use std::{collections::HashMap, env};

    use alloy_primitives::U256;
    use brontes_database::clickhouse::Clickhouse;
    use brontes_pricing::{
        types::{PoolKeyWithDirection, PoolKeysForPair, PoolStateSnapShot},
        uniswap_v2::UniswapV2Pool,
        uniswap_v3::{Info, UniswapV3Pool},
    };

    use super::*;

    fn init_clickhouse() -> Clickhouse {
        dotenv::dotenv().ok();
        let clickhouse = Clickhouse::default();

        clickhouse
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
                            Address::from_str(&"0x0000000000000000000000000000000000000000")
                                .unwrap(),
                            Address::from_str(&"0x00000000a000000000000a0000000000000a0000")
                                .unwrap(),
                        ),
                        Default::default(),
                    );
                    map.insert(
                        Pair(
                            Address::from_str(&"0x0000000000000000000000000000000000000000")
                                .unwrap(),
                            Address::from_str(&"0x00000000a000000000000a0000000000000a0000")
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
                            Address::from_str(&"0x2000000000000000000000000000000000000000")
                                .unwrap(),
                            Address::from_str(&"0x10000000a000000000000a0000000000000a0000")
                                .unwrap(),
                        ),
                        Default::default(),
                    );
                    map.insert(
                        Pair(
                            Address::from_str(&"0x0000000000000011110000000000000000000000")
                                .unwrap(),
                            Address::from_str(&"0xef000000a000002200000a0000000000000a0000")
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
