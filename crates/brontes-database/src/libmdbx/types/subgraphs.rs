use std::collections::HashMap;

use alloy_rlp::{Decodable, Encodable};
use brontes_pricing::{
    PoolPairInfoDirection, PoolPairInformation, Protocol, SubGraphEdge, SubGraphsEntry,
};
use brontes_types::{db::redefined_types::primitives::Redefined_Address, extra_processing::Pair};
use bytes::BufMut;
use redefined::{Redefined, RedefinedConvert};
use reth_db::{
    table::{Compress, Decompress},
    DatabaseError,
};
use rkyv::Deserialize;
use sorella_db_databases::clickhouse::{self, Row};

use super::{CompressedTable, LibmdbxData};
use crate::libmdbx::SubGraphs;

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, Row)]
pub struct SubGraphsData {
    pub pair: Pair,
    pub data: SubGraphsEntry,
}

impl LibmdbxData<SubGraphs> for SubGraphsData {
    fn into_key_val(
        &self,
    ) -> (
        <SubGraphs as reth_db::table::Table>::Key,
        <SubGraphs as CompressedTable>::DecompressedValue,
    ) {
        (self.pair, self.data.clone())
    }
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
#[redefined(SubGraphsEntry)]
pub struct LibmdbxSubGraphsEntry(pub HashMap<u64, Vec<LibmdbxSubGraphEdge>>);

impl Encodable for LibmdbxSubGraphsEntry {
    fn encode(&self, out: &mut dyn BufMut) {
        let encoded = rkyv::to_bytes::<_, 256>(self).unwrap();

        out.put_slice(&encoded)
    }
}

impl Decodable for LibmdbxSubGraphsEntry {
    fn decode(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        let archived: &ArchivedLibmdbxSubGraphsEntry = unsafe { rkyv::archived_root::<Self>(buf) };

        let this = archived.deserialize(&mut rkyv::Infallible).unwrap();

        Ok(this)
    }
}

impl Compress for LibmdbxSubGraphsEntry {
    type Compressed = Vec<u8>;

    fn compress_to_buf<B: reth_primitives::bytes::BufMut + AsMut<[u8]>>(self, buf: &mut B) {
        let mut encoded = Vec::new();
        self.encode(&mut encoded);
        let encoded_compressed = zstd::encode_all(&*encoded, 0).unwrap();

        buf.put_slice(&encoded_compressed);
    }
}

impl Decompress for LibmdbxSubGraphsEntry {
    fn decompress<B: AsRef<[u8]>>(value: B) -> Result<Self, reth_db::DatabaseError> {
        let binding = value.as_ref().to_vec();

        let encoded_decompressed = zstd::decode_all(&*binding).unwrap();
        let buf = &mut encoded_decompressed.as_slice();

        LibmdbxSubGraphsEntry::decode(buf).map_err(|_| DatabaseError::Decode)
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
pub struct LibmdbxPoolPairInformation {
    pub pool_addr: Redefined_Address,
    pub dex_type:  Protocol,
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
pub struct LibmdbxPoolPairInfoDirection {
    pub info:       LibmdbxPoolPairInformation,
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
pub struct LibmdbxSubGraphEdge {
    pub info:                   LibmdbxPoolPairInfoDirection,
    pub distance_to_start_node: u8,
    pub distance_to_end_node:   u8,
}
