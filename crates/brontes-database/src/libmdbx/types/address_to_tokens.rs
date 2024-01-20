use alloy_rlp::{Decodable, Encodable};
use brontes_types::{
    db::{address_to_tokens::PoolTokens, redefined_types::primitives::Redefined_Address},
    serde_utils::primitives::address_string,
};
use bytes::BufMut;
use redefined::{Redefined, RedefinedConvert};
use reth_db::{
    table::{Compress, Decompress},
    DatabaseError,
};
use reth_primitives::Address;
use rkyv::Deserialize;
use serde_with::serde_as;
use sorella_db_databases::clickhouse::{self, Row};

use super::{CompressedTable, LibmdbxData};
use crate::libmdbx::{types::utils::pool_tokens, AddressToTokens};

#[serde_as]
#[derive(Debug, Clone, Row, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AddressToTokensData {
    #[serde(with = "address_string")]
    pub address: Address,
    #[serde(with = "pool_tokens")]
    pub tokens:  PoolTokens,
}

impl LibmdbxData<AddressToTokens> for AddressToTokensData {
    fn into_key_val(
        &self,
    ) -> (
        <AddressToTokens as reth_db::table::Table>::Key,
        <AddressToTokens as CompressedTable>::DecompressedValue,
    ) {
        (self.address, self.tokens.clone())
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
#[redefined(PoolTokens)]
pub struct LibmdbxPoolTokens {
    pub token0:     Redefined_Address,
    pub token1:     Redefined_Address,
    pub token2:     Option<Redefined_Address>,
    pub token3:     Option<Redefined_Address>,
    pub token4:     Option<Redefined_Address>,
    pub init_block: u64,
}
impl Encodable for LibmdbxPoolTokens {
    fn encode(&self, out: &mut dyn BufMut) {
        let encoded = rkyv::to_bytes::<_, 256>(self).unwrap();

        out.put_slice(&encoded)
    }
}

impl Decodable for LibmdbxPoolTokens {
    fn decode(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        let archived: &ArchivedLibmdbxPoolTokens = unsafe { rkyv::archived_root::<Self>(buf) };

        let this = archived.deserialize(&mut rkyv::Infallible).unwrap();

        Ok(this)
    }
}

impl Compress for LibmdbxPoolTokens {
    type Compressed = Vec<u8>;

    fn compress_to_buf<B: reth_primitives::bytes::BufMut + AsMut<[u8]>>(self, buf: &mut B) {
        let mut encoded = Vec::new();
        self.encode(&mut encoded);
        let encoded_compressed = zstd::encode_all(&*encoded, 0).unwrap();

        buf.put_slice(&encoded_compressed);
    }
}

impl Decompress for LibmdbxPoolTokens {
    fn decompress<B: AsRef<[u8]>>(value: B) -> Result<Self, reth_db::DatabaseError> {
        let binding = value.as_ref().to_vec();

        let encoded_decompressed = zstd::decode_all(&*binding).unwrap();
        let buf = &mut encoded_decompressed.as_slice();

        LibmdbxPoolTokens::decode(buf).map_err(|_| DatabaseError::Decode)
    }
}
