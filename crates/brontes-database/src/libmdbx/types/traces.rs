use alloy_rlp::{Decodable, Encodable};
pub use brontes_types::extra_processing::Pair;
use brontes_types::structured_trace::TxTrace;
use bytes::BufMut;
use reth_db::{
    table::{Compress, Decompress},
    DatabaseError,
};
use reth_primitives::{Address, TxHash, U256};
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use sorella_db_databases::{clickhouse, Row};

use super::{
    utils::{option_address, u256},
    LibmdbxData,
};
use crate::tables::{Metadata, TxTracesDB};

#[serde_as]
#[derive(Debug, Clone, Row, Serialize, Deserialize)]
pub struct TxTracesDBData {
    pub block_number: u64,
    //#[serde(flatten)]
    pub inner:        TxTracesInner,
}

impl TxTracesDBData {
    pub fn new(block_number: u64, inner: TxTracesInner) -> Self {
        Self { block_number, inner }
    }
}

impl LibmdbxData<TxTracesDB> for TxTracesDBData {
    fn into_key_val(
        &self,
    ) -> (<TxTracesDB as reth_db::table::Table>::Key, <TxTracesDB as reth_db::table::Table>::Value)
    {
        (self.block_number, self.inner.clone())
    }
}

#[serde_as]
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct TxTracesInner {
    pub traces: Option<Vec<TxTrace>>,
}

impl TxTracesInner {
    pub fn new(traces: Option<Vec<TxTrace>>) -> Self {
        Self { traces }
    }
}

impl Encodable for TxTracesInner {
    fn encode(&self, out: &mut dyn BufMut) {
        serde_json::to_value(self)
            .unwrap()
            .to_string()
            .as_bytes()
            .encode(out);
    }
}

impl Decodable for TxTracesInner {
    fn decode(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        let this = serde_json::from_str(&String::decode(buf)?).unwrap();

        Ok(this)
    }
}

impl Compress for TxTracesInner {
    type Compressed = Vec<u8>;

    fn compress_to_buf<B: reth_primitives::bytes::BufMut + AsMut<[u8]>>(self, buf: &mut B) {
        let mut encoded = Vec::new();
        self.encode(&mut encoded);
        buf.put_slice(&encoded);
    }
}

impl Decompress for TxTracesInner {
    fn decompress<B: AsRef<[u8]>>(value: B) -> Result<Self, reth_db::DatabaseError> {
        let binding = value.as_ref().to_vec();
        let buf = &mut binding.as_slice();
        TxTracesInner::decode(buf).map_err(|_| DatabaseError::Decode)
    }
}
