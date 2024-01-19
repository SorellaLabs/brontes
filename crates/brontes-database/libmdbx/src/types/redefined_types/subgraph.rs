use std::collections::HashMap;

use alloy_rlp::{Decodable, Encodable};
use brontes_pricing::{PoolPairInfoDirection, PoolPairInformation, SubGraphEdge};
use brontes_types::{
    exchanges::StaticBindingsDb, libmdbx::redefined_types::primitives::Redefined_Address,
};
use bytes::BufMut;
use redefined::{Redefined, RedefinedConvert};
use reth_db::{
    table::{Compress, Decompress},
    DatabaseError,
};
use rkyv::Deserialize;

use crate::types::subgraphs::SubGraphsEntry;

#[derive(
    Debug,
    Clone,
    serde::Serialize,
    serde::Deserialize,
    rkyv::Serialize,
    rkyv::Deserialize,
    rkyv::Archive,
    Redefined,
)]
#[redefined(SubGraphsEntry)]
pub struct Redefined_SubGraphsEntry(pub HashMap<u64, Vec<Redefined_SubGraphEdge>>);

impl Encodable for Redefined_SubGraphsEntry {
    fn encode(&self, out: &mut dyn BufMut) {
        let encoded = rkyv::to_bytes::<_, 256>(self).unwrap();

        out.put_slice(&encoded)
    }
}

impl Decodable for Redefined_SubGraphsEntry {
    fn decode(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        let archived: &ArchivedRedefined_SubGraphsEntry =
            unsafe { rkyv::archived_root::<Self>(buf) };

        let this = archived.deserialize(&mut rkyv::Infallible).unwrap();

        Ok(this)
    }
}

impl Compress for Redefined_SubGraphsEntry {
    type Compressed = Vec<u8>;

    fn compress_to_buf<B: reth_primitives::bytes::BufMut + AsMut<[u8]>>(self, buf: &mut B) {
        let mut encoded = Vec::new();
        self.encode(&mut encoded);
        let encoded_compressed = zstd::encode_all(&*encoded, 0).unwrap();

        buf.put_slice(&encoded_compressed);
    }
}

impl Decompress for Redefined_SubGraphsEntry {
    fn decompress<B: AsRef<[u8]>>(value: B) -> Result<Self, reth_db::DatabaseError> {
        let binding = value.as_ref().to_vec();

        let encoded_decompressed = zstd::decode_all(&*binding).unwrap();
        let buf = &mut encoded_decompressed.as_slice();

        Redefined_SubGraphsEntry::decode(buf).map_err(|_| DatabaseError::Decode)
    }
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    rkyv::Serialize,
    rkyv::Deserialize,
    rkyv::Archive,
    Redefined,
)]
#[redefined(PoolPairInformation)]
pub struct Redefined_PoolPairInformation {
    pub pool_addr: Redefined_Address,
    pub dex_type:  StaticBindingsDb,
    pub token_0:   Redefined_Address,
    pub token_1:   Redefined_Address,
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    rkyv::Serialize,
    rkyv::Deserialize,
    rkyv::Archive,
    Redefined,
)]
#[redefined(PoolPairInfoDirection)]
pub struct Redefined_PoolPairInfoDirection {
    pub info:       Redefined_PoolPairInformation,
    pub token_0_in: bool,
}

#[derive(
    Debug,
    Clone,
    serde::Serialize,
    serde::Deserialize,
    rkyv::Serialize,
    rkyv::Deserialize,
    rkyv::Archive,
    Redefined,
)]
#[redefined(SubGraphEdge)]
pub struct Redefined_SubGraphEdge {
    pub info:                   Redefined_PoolPairInfoDirection,
    pub distance_to_start_node: u8,
    pub distance_to_end_node:   u8,
}
