use std::collections::HashMap;

use alloy_rlp::{Decodable, Encodable};
use brontes_types::libmdbx::redefined_types::{
    malachite::Redefined_Rational, primitives::Redefined_Address,
};
use bytes::BufMut;
use redefined::{Redefined, RedefinedConvert};
use reth_db::{
    table::{Compress, Decompress},
    DatabaseError,
};
use rkyv::Deserialize;

use super::price_maps::Redefined_Pair;
use crate::types::cex_price::{CexPriceMap, CexQuote};

#[derive(
    Debug, Clone, serde::Serialize, rkyv::Serialize, rkyv::Deserialize, rkyv::Archive, Redefined,
)]
#[redefined(CexPriceMap)]
pub struct Redefined_CexPriceMap(pub HashMap<Redefined_Pair, Vec<Redefined_CexQuote>>);

impl Encodable for Redefined_CexPriceMap {
    fn encode(&self, out: &mut dyn BufMut) {
        let encoded = rkyv::to_bytes::<_, 256>(self).unwrap();

        out.put_slice(&encoded);
    }
}

impl Decodable for Redefined_CexPriceMap {
    fn decode(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        let archived: &ArchivedRedefined_CexPriceMap = unsafe { rkyv::archived_root::<Self>(buf) };

        let this = archived.deserialize(&mut rkyv::Infallible).unwrap();

        Ok(this)
    }
}

impl Compress for Redefined_CexPriceMap {
    type Compressed = Vec<u8>;

    fn compress_to_buf<B: reth_primitives::bytes::BufMut + AsMut<[u8]>>(self, buf: &mut B) {
        let mut encoded = Vec::new();
        self.encode(&mut encoded);
        let encoded_compressed = zstd::encode_all(&*encoded, 0).unwrap();

        buf.put_slice(&encoded_compressed);
    }
}

impl Decompress for Redefined_CexPriceMap {
    fn decompress<B: AsRef<[u8]>>(value: B) -> Result<Self, reth_db::DatabaseError> {
        let binding = value.as_ref().to_vec();

        let encoded_decompressed = zstd::decode_all(&*binding).unwrap();
        let buf = &mut encoded_decompressed.as_slice();

        Redefined_CexPriceMap::decode(buf).map_err(|_| DatabaseError::Decode)
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
pub struct Redefined_CexQuote {
    pub exchange:  Option<String>,
    pub timestamp: u64,
    pub price:     (Redefined_Rational, Redefined_Rational),
    pub token0:    Redefined_Address,
}

impl PartialEq for Redefined_CexQuote {
    fn eq(&self, other: &Self) -> bool {
        self.clone().to_source().eq(&other.clone().to_source())
    }
}
