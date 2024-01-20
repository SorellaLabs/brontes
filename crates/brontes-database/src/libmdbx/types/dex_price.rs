

use alloy_primitives::TxHash;
use alloy_rlp::{Decodable, Encodable};
use brontes_types::db::{
        dex::{DexQuote, DexQuoteWithIndex},
        redefined_types::{malachite::Redefined_Rational, primitives::Redefined_Pair},
    };
use bytes::BufMut;

use redefined::{Redefined, RedefinedConvert};
use reth_db::{
    table::{Compress, Decompress},
    DatabaseError,
};
use rkyv::Deserialize;
use sorella_db_databases::clickhouse::{self, Row};

use super::{CompressedTable, LibmdbxData};
use crate::libmdbx::DexPrice;

#[derive(Debug, Clone, Row, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DexPriceData {
    pub block_number: u64,
    pub tx_idx:       u16,
    pub quote:        DexQuote,
}

impl LibmdbxData<DexPrice> for DexPriceData {
    fn into_key_val(
        &self,
    ) -> (<DexPrice as reth_db::table::Table>::Key, <DexPrice as CompressedTable>::DecompressedValue)
    {
        (
            make_key(self.block_number, self.tx_idx),
            DexQuoteWithIndex { tx_idx: self.tx_idx, quote: self.quote.clone().into() },
        )
    }
}

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

#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
    rkyv::Archive,
    rkyv::Deserialize,
    rkyv::Serialize,
    Redefined,
)]
#[redefined(DexQuoteWithIndex)]
pub struct LibmdbxDexQuoteWithIndex {
    pub tx_idx: u16,
    pub quote:  Vec<(Redefined_Pair, Redefined_Rational)>,
}

impl Encodable for LibmdbxDexQuoteWithIndex {
    fn encode(&self, out: &mut dyn BufMut) {
        let encoded = rkyv::to_bytes::<_, 256>(self).unwrap();

        out.put_slice(&encoded)
    }
}

impl Decodable for LibmdbxDexQuoteWithIndex {
    fn decode(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        let archived: &ArchivedLibmdbxDexQuoteWithIndex =
            unsafe { rkyv::archived_root::<Self>(buf) };

        let this = archived.deserialize(&mut rkyv::Infallible).unwrap();

        Ok(this)
    }
}

impl Compress for LibmdbxDexQuoteWithIndex {
    type Compressed = Vec<u8>;

    fn compress_to_buf<B: reth_primitives::bytes::BufMut + AsMut<[u8]>>(self, buf: &mut B) {
        let mut encoded = Vec::new();
        self.encode(&mut encoded);
        let encoded_compressed = zstd::encode_all(&*encoded, 0).unwrap();

        buf.put_slice(&encoded_compressed);
    }
}

impl Decompress for LibmdbxDexQuoteWithIndex {
    fn decompress<B: AsRef<[u8]>>(value: B) -> Result<Self, reth_db::DatabaseError> {
        let binding = value.as_ref().to_vec();

        let encoded_decompressed = zstd::decode_all(&*binding).unwrap();
        let buf = &mut encoded_decompressed.as_slice();

        LibmdbxDexQuoteWithIndex::decode(buf).map_err(|_| DatabaseError::Decode)
    }
}

/*
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
*/
