use alloy_rlp::{Decodable, Encodable};
use brontes_types::db::{
    metadata::MetadataInner,
    redefined_types::primitives::{Redefined_Address, Redefined_TxHash, Redefined_U256},
};
use bytes::BufMut;
use redefined::{Redefined, RedefinedConvert};
use reth_db::{
    table::{Compress, Decompress},
    DatabaseError,
};
use rkyv::Deserialize;
use serde_with::serde_as;
use sorella_db_databases::{clickhouse, clickhouse::Row};

use super::{CompressedTable, LibmdbxData};
use crate::libmdbx::Metadata;

#[serde_as]
#[derive(Debug, Clone, Row, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct MetadataData {
    pub block_number: u64,
    pub inner:        MetadataInner,
}

impl LibmdbxData<Metadata> for MetadataData {
    fn into_key_val(
        &self,
    ) -> (<Metadata as reth_db::table::Table>::Key, <Metadata as CompressedTable>::DecompressedValue)
    {
        (self.block_number, self.inner.clone())
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
#[redefined(MetadataInner)]
pub struct LibmdbxMetadataInner {
    pub block_hash:             Redefined_U256,
    pub block_timestamp:        u64,
    pub relay_timestamp:        Option<u64>,
    pub p2p_timestamp:          Option<u64>,
    pub proposer_fee_recipient: Option<Redefined_Address>,
    pub proposer_mev_reward:    Option<u128>,
    pub mempool_flow:           Vec<Redefined_TxHash>,
}
impl Encodable for LibmdbxMetadataInner {
    fn encode(&self, out: &mut dyn BufMut) {
        let encoded = rkyv::to_bytes::<_, 256>(self).unwrap();

        out.put_slice(&encoded)
    }
}

impl Decodable for LibmdbxMetadataInner {
    fn decode(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        let archived: &ArchivedLibmdbxMetadataInner = unsafe { rkyv::archived_root::<Self>(buf) };

        let this = archived.deserialize(&mut rkyv::Infallible).unwrap();

        Ok(this)
    }
}

impl Compress for LibmdbxMetadataInner {
    type Compressed = Vec<u8>;

    fn compress_to_buf<B: reth_primitives::bytes::BufMut + AsMut<[u8]>>(self, buf: &mut B) {
        let mut encoded = Vec::new();
        self.encode(&mut encoded);
        let encoded_compressed = zstd::encode_all(&*encoded, 0).unwrap();

        buf.put_slice(&encoded_compressed);
    }
}

impl Decompress for LibmdbxMetadataInner {
    fn decompress<B: AsRef<[u8]>>(value: B) -> Result<Self, reth_db::DatabaseError> {
        let binding = value.as_ref().to_vec();

        let encoded_decompressed = zstd::decode_all(&*binding).unwrap();
        let buf = &mut encoded_decompressed.as_slice();

        LibmdbxMetadataInner::decode(buf).map_err(|_| DatabaseError::Decode)
    }
}
