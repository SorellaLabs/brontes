use alloy_rlp::{Decodable, Encodable};
use bytes::BufMut;
use reth_db::{
    table::{Compress, Decompress},
    DatabaseError,
};
use serde::{Deserialize, Serialize};

use crate::structured_trace::TxTrace;

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
