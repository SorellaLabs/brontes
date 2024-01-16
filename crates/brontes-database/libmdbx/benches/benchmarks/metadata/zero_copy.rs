use std::str::FromStr;

use alloy_rlp::{Decodable, Encodable};
use brontes_database_libmdbx::types::{utils::*, LibmdbxData};
use bytes::{BufMut, Bytes, BytesMut};
use reth_db::{
    table::{Compress, Decompress},
    DatabaseError,
};
use reth_primitives::{Address, TxHash, U256};
use rkyv::{
    ser::{ScratchSpace, Serializer},
    vec::{ArchivedVec, VecResolver},
    Archive, Archived, Deserialize, Fallible, Infallible, Serialize,
};
use serde_with::{serde_as, DisplayFromStr};
use sorella_db_databases::{clickhouse, Row};

use super::MetadataBench;
use crate::setup::tables::MetadataRkyv;

#[serde_as]
#[derive(Debug, Clone, Row, serde::Serialize, serde::Deserialize)]
pub struct MetadataRkyvData {
    pub block_number: u64,
    //#[serde(flatten)]
    pub inner:        MetadataRkyvInner,
}

impl LibmdbxData<MetadataRkyv> for MetadataRkyvData {
    fn into_key_val(
        &self,
    ) -> (
        <MetadataRkyv as reth_db::table::Table>::Key,
        <MetadataRkyv as reth_db::table::Table>::Value,
    ) {
        (self.block_number, self.inner.clone())
    }
}

#[derive(
    Debug, Default, Clone, serde::Serialize, serde::Deserialize, Deserialize, Serialize, Archive,
)]
pub struct TxHashOwned(pub [u8; 32]);
/*
impl rkyv::Archive for TxHashOwned {
    type Archived = ArchivedVec<u8>;
    type Resolver = VecResolver;

    unsafe fn resolve(&self, pos: usize, resolver: Self::Resolver, out: *mut Self::Archived) {
        ArchivedVec::resolve_from_slice(&self.0 .0, pos, resolver, out);
    }
}

impl<S: Fallible + ?Sized + Serializer + ScratchSpace> Serialize<S> for TxHashOwned {
    fn serialize(&self, serializer: &mut S) -> Result<Self::Resolver, S::Error> {
        ArchivedVec::serialize_from_slice(&self.0 .0, serializer)
    }
}
*/
#[derive(
    Debug, Default, Clone, serde::Serialize, serde::Deserialize, Deserialize, Serialize, Archive,
)]
pub struct AddressOwned(pub [u8; 20]);

/*
impl rkyv::Archive for AddressOwned {
    type Archived = ArchivedVec<u8>;
    type Resolver = VecResolver;

    unsafe fn resolve(&self, pos: usize, resolver: Self::Resolver, out: *mut Self::Archived) {
        ArchivedVec::resolve_from_slice(&self.0 .0 .0, pos, resolver, out);
    }
}

impl<S: Fallible + ?Sized + Serializer + ScratchSpace> Serialize<S> for AddressOwned {
    fn serialize(&self, serializer: &mut S) -> Result<Self::Resolver, S::Error> {
        ArchivedVec::serialize_from_slice(&self.0 .0 .0, serializer)
    }
}
*/
#[derive(
    Debug, Default, Clone, serde::Serialize, serde::Deserialize, Deserialize, Serialize, Archive,
)]
pub struct U256Owned(pub [u8; 32]);

/*
impl rkyv::Archive for U256Owned {
    type Archived = ArchivedVec<u8>;
    type Resolver = VecResolver;

    unsafe fn resolve(&self, pos: usize, resolver: Self::Resolver, out: *mut Self::Archived) {
        ArchivedVec::resolve_from_slice(&self.0, pos, resolver, out);
    }
}

impl<S: Fallible + ?Sized + Serializer + ScratchSpace> Serialize<S> for U256Owned {
    fn serialize(&self, serializer: &mut S) -> Result<Self::Resolver, S::Error> {
        ArchivedVec::serialize_from_slice(&self.0.as_le_bytes(), serializer)
    }
}
*/
#[serde_as]
#[derive(
    Debug, Default, Clone, serde::Serialize, serde::Deserialize, Serialize, Deserialize, Archive,
)]
//#[archive(check_bytes)]
pub struct MetadataRkyvInner {
    pub block_hash:             U256Owned,
    pub block_timestamp:        u64,
    pub relay_timestamp:        Option<u64>,
    pub p2p_timestamp:          Option<u64>,
    pub proposer_fee_recipient: Option<AddressOwned>,
    pub proposer_mev_reward:    Option<u128>,
    pub mempool_flow:           Vec<TxHashOwned>,
}

impl Encodable for MetadataRkyvInner {
    fn encode(&self, out: &mut dyn BufMut) {
        let bytes = rkyv::to_bytes::<_, 256>(self).unwrap();

        out.put_slice(&bytes)
    }
}

impl Decodable for MetadataRkyvInner {
    fn decode(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        let archived: &ArchivedMetadataRkyvInner = unsafe { rkyv::archived_root::<Self>(buf) };

        let this = archived.deserialize(&mut rkyv::Infallible).unwrap();

        Ok(this)
    }
}

impl Compress for MetadataRkyvInner {
    type Compressed = Vec<u8>;

    fn compress_to_buf<B: reth_primitives::bytes::BufMut + AsMut<[u8]>>(self, buf: &mut B) {
        let mut encoded = Vec::new();

        self.encode(&mut encoded);

        /*

                let block_hash =
            U256::from_str("0x10a27d25828e24f7b12257bbedda621a6d94f01a2f06fee4828d931027992283")
                .unwrap();
        if block_hash.to_le_bytes() == self.block_hash.0 {
            println!("RKYV COMPRESSED LEN: {}", encoded.len());
        }
        */
        buf.put_slice(&encoded);
    }
}

impl Decompress for MetadataRkyvInner {
    fn decompress<B: AsRef<[u8]>>(value: B) -> Result<Self, reth_db::DatabaseError> {
        let binding = value.as_ref().to_vec();
        let buf = &mut binding.as_slice();
        MetadataRkyvInner::decode(buf).map_err(|_| DatabaseError::Decode)
    }
}

impl From<MetadataBench> for MetadataRkyvData {
    fn from(value: MetadataBench) -> Self {
        MetadataRkyvData {
            block_number: value.block_number,
            inner:        MetadataRkyvInner {
                block_hash:             U256Owned(value.block_hash.to_le_bytes()),
                block_timestamp:        value.block_timestamp,
                relay_timestamp:        value.relay_timestamp,
                p2p_timestamp:          value.p2p_timestamp,
                proposer_fee_recipient: value
                    .proposer_fee_recipient
                    .map(|val| AddressOwned(val.0 .0)),
                proposer_mev_reward:    value.proposer_mev_reward,
                mempool_flow:           value
                    .mempool_flow
                    .into_iter()
                    .map(|val| TxHashOwned(val.0))
                    .collect(),
            },
        }
    }
}
