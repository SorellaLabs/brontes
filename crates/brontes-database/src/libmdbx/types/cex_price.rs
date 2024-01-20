use std::collections::HashMap;

use alloy_rlp::{Decodable, Encodable};
use brontes_types::db::{
    cex::{CexPriceMap, CexQuote},
    redefined_types::{
        malachite::Redefined_Rational,
        primitives::{Redefined_Address, Redefined_Pair},
    },
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
use crate::libmdbx::CexPrice;

#[derive(Debug, Clone, Row, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CexPriceData {
    pub block_num:     u64,
    pub cex_price_map: CexPriceMap,
}

impl LibmdbxData<CexPrice> for CexPriceData {
    fn into_key_val(
        &self,
    ) -> (<CexPrice as reth_db::table::Table>::Key, <CexPrice as CompressedTable>::DecompressedValue)
    {
        (self.block_num, self.cex_price_map.clone())
    }
}

#[derive(
    Debug, Clone, serde::Serialize, rkyv::Serialize, rkyv::Deserialize, rkyv::Archive, Redefined,
)]
#[redefined(CexPriceMap)]
pub struct LibmdbxCexPriceMap(pub HashMap<Redefined_Pair, Vec<LibmdbxCexQuote>>);

impl Encodable for LibmdbxCexPriceMap {
    fn encode(&self, out: &mut dyn BufMut) {
        let encoded = rkyv::to_bytes::<_, 256>(self).unwrap();

        out.put_slice(&encoded);
    }
}

impl Decodable for LibmdbxCexPriceMap {
    fn decode(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        let archived: &ArchivedLibmdbxCexPriceMap = unsafe { rkyv::archived_root::<Self>(buf) };

        let this = archived.deserialize(&mut rkyv::Infallible).unwrap();

        Ok(this)
    }
}

impl Compress for LibmdbxCexPriceMap {
    type Compressed = Vec<u8>;

    fn compress_to_buf<B: reth_primitives::bytes::BufMut + AsMut<[u8]>>(self, buf: &mut B) {
        let mut encoded = Vec::new();
        self.encode(&mut encoded);
        let encoded_compressed = zstd::encode_all(&*encoded, 0).unwrap();

        buf.put_slice(&encoded_compressed);
    }
}

impl Decompress for LibmdbxCexPriceMap {
    fn decompress<B: AsRef<[u8]>>(value: B) -> Result<Self, reth_db::DatabaseError> {
        let binding = value.as_ref().to_vec();

        let encoded_decompressed = zstd::decode_all(&*binding).unwrap();
        let buf = &mut encoded_decompressed.as_slice();

        LibmdbxCexPriceMap::decode(buf).map_err(|_| DatabaseError::Decode)
    }
}

#[derive(
    Debug,
    Clone,
    Hash,
    Eq,
    serde::Serialize,
    rkyv::Serialize,
    rkyv::Deserialize,
    rkyv::Archive,
    Redefined,
)]
#[redefined(CexQuote)]
pub struct LibmdbxCexQuote {
    pub exchange:  Option<String>,
    pub timestamp: u64,
    pub price:     (Redefined_Rational, Redefined_Rational),
    pub token0:    Redefined_Address,
}

impl PartialEq for LibmdbxCexQuote {
    fn eq(&self, other: &Self) -> bool {
        self.clone().to_source().eq(&other.clone().to_source())
    }
}
