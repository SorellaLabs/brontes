use alloy_rlp::{Decodable, Encodable};
use brontes_database_libmdbx::types::{utils::*, LibmdbxData};
pub use brontes_types::extra_processing::Pair;
use bytes::BufMut;
use reth_db::{
    table::{Compress, Decompress},
    DatabaseError,
};
use reth_primitives::{Address, TxHash, U256};
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use sorella_db_databases::{clickhouse, Row};

use super::MetadataBench;
use crate::{
    bench_table,
    benchmarks::tables::{InitializeTable, IntoTableKey},
};

#[serde_as]
#[derive(Debug, Clone, Row,  Serialize, Deserialize)]
pub struct MetadataRLPData {
    pub block_number: u64,
    //#[serde(flatten)]
    pub inner:        MetadataRLPInner,
}

impl LibmdbxData<MetadataRLP> for MetadataRLPData {
    fn into_key_val(
        &self,
    ) -> (<MetadataRLP as reth_db::table::Table>::Key, <MetadataRLP as reth_db::table::Table>::Value)
    {
        (self.block_number, self.inner.clone())
    }
}

#[serde_as]
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct MetadataRLPInner {
    #[serde(with = "u256")]
    pub block_hash:             U256,
    pub block_timestamp:        u64,
    pub relay_timestamp:        Option<u64>,
    pub p2p_timestamp:          Option<u64>,
    #[serde(with = "option_address")]
    pub proposer_fee_recipient: Option<Address>,
    pub proposer_mev_reward:    Option<u128>,
    #[serde_as(as = "Vec<DisplayFromStr>")]
    pub mempool_flow:           Vec<TxHash>,
}

impl Encodable for MetadataRLPInner {
    fn encode(&self, out: &mut dyn BufMut) {
        self.block_hash.encode(out);
        self.block_timestamp.encode(out);
        self.relay_timestamp.unwrap_or_default().encode(out);
        self.p2p_timestamp.unwrap_or_default().encode(out);
        self.proposer_fee_recipient.unwrap_or_default().encode(out);
        self.proposer_mev_reward.unwrap_or_default().encode(out);
        self.mempool_flow.encode(out);
    }
}

impl Decodable for MetadataRLPInner {
    fn decode(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        let block_hash = U256::decode(buf)?;
        let block_timestamp = u64::decode(buf)?;
        let mut relay_timestamp = Some(u64::decode(buf)?);
        if relay_timestamp.as_ref().unwrap() == &0 {
            relay_timestamp = None
        }
        let mut p2p_timestamp = Some(u64::decode(buf)?);
        if p2p_timestamp.as_ref().unwrap() == &0 {
            p2p_timestamp = None
        }
        let mut proposer_fee_recipient = Some(Address::decode(buf)?);
        if proposer_fee_recipient.as_ref().unwrap().is_zero() {
            proposer_fee_recipient = None
        }
        let mut proposer_mev_reward = Some(u128::decode(buf)?);
        if proposer_mev_reward.as_ref().unwrap() == &0 {
            proposer_mev_reward = None
        }
        let mempool_flow = Vec::<TxHash>::decode(buf)?;

        Ok(Self {
            block_hash,
            block_timestamp,
            relay_timestamp,
            p2p_timestamp,
            proposer_fee_recipient,
            proposer_mev_reward,
            mempool_flow,
        })
    }
}

impl Compress for MetadataRLPInner {
    type Compressed = Vec<u8>;

    fn compress_to_buf<B: reth_primitives::bytes::BufMut + AsMut<[u8]>>(self, buf: &mut B) {
        let mut encoded = Vec::new();
        self.encode(&mut encoded);
        buf.put_slice(&encoded);
    }
}

impl Decompress for MetadataRLPInner {
    fn decompress<B: AsRef<[u8]>>(value: B) -> Result<Self, reth_db::DatabaseError> {
        let binding = value.as_ref().to_vec();
        let buf = &mut binding.as_slice();
        MetadataRLPInner::decode(buf).map_err(|_| DatabaseError::Decode)
    }
}


impl From<MetadataBench> for MetadataRLPData {
    fn from(value: MetadataBench) -> Self {
        MetadataRLPData {
            block_number: value.block_number,
            inner: MetadataRLPInner {
                block_hash: value.block_hash,
                block_timestamp: value.block_timestamp,
                relay_timestamp: value.relay_timestamp,
                p2p_timestamp: value.p2p_timestamp,
                proposer_fee_recipient: value.proposer_fee_recipient,
                proposer_mev_reward: value.proposer_mev_reward,
                mempool_flow: value.mempool_flow,
            }
        }
    }
}

bench_table!(
    /// rlp metadata
    ( MetadataRLP ) u64 | MetadataRLPInner | MetadataBench
);
