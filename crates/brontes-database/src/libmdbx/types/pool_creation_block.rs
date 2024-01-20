use alloy_primitives::Address;
use alloy_rlp::{Decodable, Encodable};
use brontes_types::db::{
    pool_creation_block::PoolsToAddresses, redefined_types::primitives::Redefined_Address,
};
use bytes::BufMut;
use redefined::{Redefined, RedefinedConvert};
use reth_db::{
    table::{Compress, Decompress},
    DatabaseError,
};
use rkyv::Deserialize;
use serde_with::serde_as;
use sorella_db_databases::clickhouse::{self, Row};

use super::{utils::pools_libmdbx, CompressedTable, LibmdbxData};
use crate::libmdbx::PoolCreationBlocks;

#[serde_as]
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, Row)]
pub struct PoolCreationBlocksData {
    pub block_number: u64,
    #[serde(with = "pools_libmdbx")]
    pub pools:        PoolsToAddresses,
}

impl LibmdbxData<PoolCreationBlocks> for PoolCreationBlocksData {
    fn into_key_val(
        &self,
    ) -> (
        <PoolCreationBlocks as reth_db::table::Table>::Key,
        <PoolCreationBlocks as CompressedTable>::DecompressedValue,
    ) {
        (self.block_number, self.pools.clone().into())
    }
}

#[derive(
    Debug,
    PartialEq,
    Clone,
    serde::Serialize,
    rkyv::Serialize,
    rkyv::Deserialize,
    rkyv::Archive,
    Redefined,
)]
#[redefined(PoolsToAddresses)]
pub struct LibmdbxPoolsToAddresses(pub Vec<Redefined_Address>);
impl Encodable for LibmdbxPoolsToAddresses {
    fn encode(&self, out: &mut dyn BufMut) {
        let encoded = rkyv::to_bytes::<_, 256>(self).unwrap();

        out.put_slice(&encoded)
    }
}

impl Decodable for LibmdbxPoolsToAddresses {
    fn decode(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        let archived: &ArchivedLibmdbxPoolsToAddresses =
            unsafe { rkyv::archived_root::<Self>(buf) };

        let this = archived.deserialize(&mut rkyv::Infallible).unwrap();

        Ok(this)
    }
}

impl Compress for LibmdbxPoolsToAddresses {
    type Compressed = Vec<u8>;

    fn compress_to_buf<B: reth_primitives::bytes::BufMut + AsMut<[u8]>>(self, buf: &mut B) {
        let mut encoded = Vec::new();
        self.encode(&mut encoded);
        let encoded_compressed = zstd::encode_all(&*encoded, 0).unwrap();

        buf.put_slice(&encoded_compressed);
    }
}

impl Decompress for LibmdbxPoolsToAddresses {
    fn decompress<B: AsRef<[u8]>>(value: B) -> Result<Self, reth_db::DatabaseError> {
        let binding = value.as_ref().to_vec();

        let encoded_decompressed = zstd::decode_all(&*binding).unwrap();
        let buf = &mut encoded_decompressed.as_slice();

        LibmdbxPoolsToAddresses::decode(buf).map_err(|_| DatabaseError::Decode)
    }
}
