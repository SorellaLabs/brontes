use alloy_rlp::{Decodable, Encodable};
use brontes_types::libmdbx::redefined_types::malachite::Redefined_Rational;
use bytes::BufMut;
use redefined::{Redefined, RedefinedConvert};
use reth_db::{
    table::{Compress, Decompress},
    DatabaseError,
};
use rkyv::Deserialize;

use super::price_maps::Redefined_Pair;
use crate::types::dex_price::{DexQuote, DexQuoteWithIndex};

impl From<DexQuote> for Vec<(Redefined_Pair, Redefined_Rational)> {
    fn from(val: DexQuote) -> Self {
        val.0
            .into_iter()
            .map(|(x, y)| (Redefined_Pair::from_source(x), Redefined_Rational::from_source(y)))
            .collect()
    }
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
pub struct Redefined_DexQuoteWithIndex {
    pub tx_idx: u16,
    pub quote:  Vec<(Redefined_Pair, Redefined_Rational)>,
}

impl Encodable for Redefined_DexQuoteWithIndex {
    fn encode(&self, out: &mut dyn BufMut) {
        let encoded = rkyv::to_bytes::<_, 256>(self).unwrap();

        out.put_slice(&encoded)
    }
}

impl Decodable for Redefined_DexQuoteWithIndex {
    fn decode(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        let archived: &ArchivedRedefined_DexQuoteWithIndex =
            unsafe { rkyv::archived_root::<Self>(buf) };

        let this = archived.deserialize(&mut rkyv::Infallible).unwrap();

        Ok(this)
    }
}

impl Compress for Redefined_DexQuoteWithIndex {
    type Compressed = Vec<u8>;

    fn compress_to_buf<B: reth_primitives::bytes::BufMut + AsMut<[u8]>>(self, buf: &mut B) {
        let mut encoded = Vec::new();
        self.encode(&mut encoded);
        let encoded_compressed = zstd::encode_all(&*encoded, 0).unwrap();

        buf.put_slice(&encoded_compressed);
    }
}

impl Decompress for Redefined_DexQuoteWithIndex {
    fn decompress<B: AsRef<[u8]>>(value: B) -> Result<Self, reth_db::DatabaseError> {
        let binding = value.as_ref().to_vec();

        let encoded_decompressed = zstd::decode_all(&*binding).unwrap();
        let buf = &mut encoded_decompressed.as_slice();

        Redefined_DexQuoteWithIndex::decode(buf).map_err(|_| DatabaseError::Decode)
    }
}
