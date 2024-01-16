use std::{
    io::{BufReader, Read},
    str::FromStr,
};

use alloy_rlp::{Decodable, Encodable};
use bincode::{config, Decode as BincodeDecode, Encode as BincodeEncode};
use brontes_database_libmdbx::types::{utils::*, LibmdbxData};
use bytes::{Buf, BufMut, Bytes};
use reth_db::{
    table::{Compress, Decompress},
    DatabaseError,
};
use reth_primitives::{Address, TxHash, U256};
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};
use sorella_db_databases::{clickhouse, Row};
use zstd::{decode_all, encode_all};

use super::MetadataBench;
use crate::setup::tables::MetadataBincode;

#[serde_as]
#[derive(Debug, Clone, Row, Serialize, Deserialize)]
pub struct MetadataBincodeData {
    pub block_number: u64,
    //#[serde(flatten)]
    pub inner:        MetadataBincodeInner,
}

impl LibmdbxData<MetadataBincode> for MetadataBincodeData {
    fn into_key_val(
        &self,
    ) -> (
        <MetadataBincode as reth_db::table::Table>::Key,
        <MetadataBincode as reth_db::table::Table>::Value,
    ) {
        (self.block_number, self.inner.clone())
    }
}

pub trait EncodeMe {
    fn encode_me(self) -> Vec<u8>;
}

pub trait DecodeMe {
    fn decode_me(buf: &mut &[u8]) -> Self;
}

#[serde_as]
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct MetadataBincodeInner {
    #[serde(with = "u256")]
    pub block_hash:             U256, //32
    pub block_timestamp:        u64,
    pub relay_timestamp:        Option<u64>,
    pub p2p_timestamp:          Option<u64>,
    #[serde(with = "option_address")]
    pub proposer_fee_recipient: Option<Address>,
    pub proposer_mev_reward:    Option<u128>,
    #[serde_as(as = "Vec<DisplayFromStr>")]
    pub mempool_flow:           Vec<TxHash>,
}

impl EncodeMe for &MetadataBincodeInner {
    fn encode_me(self) -> Vec<u8> {
        let mut buf = Vec::new();
        let block_hash_bytes: [u8; 32] = self.block_hash.as_le_slice().try_into().unwrap();
        buf.put_slice(&block_hash_bytes);

        let block_timestamp_bytes = self.block_timestamp.to_le_bytes();
        buf.put_slice(&block_timestamp_bytes);

        if let Some(relay_timestamp) = self.relay_timestamp {
            buf.put_slice(&[1]);
            buf.put_slice(&relay_timestamp.to_le_bytes());
        } else {
            buf.put_slice(&[0]);
        }

        if let Some(p2p_timestamp) = self.p2p_timestamp {
            buf.put_slice(&[1]);
            buf.put_slice(&p2p_timestamp.to_le_bytes());
        } else {
            buf.put_slice(&[0]);
        }

        if let Some(proposer_fee_recipient) = self.proposer_fee_recipient {
            buf.put_slice(&[1]);
            buf.put_slice(&proposer_fee_recipient.0 .0);
        } else {
            buf.put_slice(&[0]);
        }

        if let Some(proposer_mev_reward) = self.proposer_mev_reward {
            buf.put_slice(&[1]);
            buf.put_slice(&proposer_mev_reward.to_le_bytes());
        } else {
            buf.put_slice(&[0]);
        }

        let mempool_flow_len = self.mempool_flow.len() as u16;
        let mempool_flow_len_bytes = mempool_flow_len.to_le_bytes();
        buf.put_slice(&mempool_flow_len_bytes);
        self.mempool_flow
            .iter()
            .for_each(|tx| buf.put_slice(tx.0.as_slice()));

        buf
    }
}

impl DecodeMe for MetadataBincodeInner {
    fn decode_me(buf: &mut &[u8]) -> Self {
        let mut block_hash_bytes: [u8; 32] = [0; 32];
        buf.copy_to_slice(&mut block_hash_bytes);
        let block_hash = U256::from_le_bytes(block_hash_bytes);

        let mut block_timestamp_bytes: [u8; 8] = [0; 8];
        buf.copy_to_slice(&mut block_timestamp_bytes);
        let block_timestamp = u64::from_le_bytes(block_timestamp_bytes);

        let mut opt_relay_timestamp_bytes: [u8; 1] = [0];
        buf.copy_to_slice(&mut opt_relay_timestamp_bytes);
        let mut relay_timestamp_bytes: [u8; 8] = [0; 8];
        let mut relay_timestamp = None;
        if opt_relay_timestamp_bytes == [1] {
            buf.copy_to_slice(&mut relay_timestamp_bytes);
            relay_timestamp = Some(u64::from_le_bytes(relay_timestamp_bytes))
        }

        let mut opt_p2p_timestamp_bytes: [u8; 1] = [0];
        buf.copy_to_slice(&mut opt_p2p_timestamp_bytes);
        let mut p2p_timestamp_bytes: [u8; 8] = [0; 8];
        let mut p2p_timestamp = None;
        if opt_p2p_timestamp_bytes == [1] {
            buf.copy_to_slice(&mut p2p_timestamp_bytes);
            p2p_timestamp = Some(u64::from_le_bytes(p2p_timestamp_bytes))
        }

        let mut opt_proposer_fee_recipient_bytes: [u8; 1] = [0];
        buf.copy_to_slice(&mut opt_proposer_fee_recipient_bytes);
        let mut proposer_fee_recipient_bytes: [u8; 20] = [0; 20];
        let mut proposer_fee_recipient = None;
        if opt_proposer_fee_recipient_bytes == [1] {
            buf.copy_to_slice(&mut proposer_fee_recipient_bytes);
            proposer_fee_recipient = Some(proposer_fee_recipient_bytes.into())
        }

        let mut opt_proposer_mev_reward_bytes: [u8; 1] = [0];
        buf.copy_to_slice(&mut opt_proposer_mev_reward_bytes);
        let mut proposer_mev_reward_bytes: [u8; 16] = [0; 16];
        let mut proposer_mev_reward = None;
        if opt_proposer_mev_reward_bytes == [1] {
            buf.copy_to_slice(&mut proposer_mev_reward_bytes);
            proposer_mev_reward = Some(u128::from_le_bytes(proposer_mev_reward_bytes))
        }

        let mut mempool_flow = Vec::new();
        let mut mempool_flow_len_bytes: [u8; 2] = [0; 2];
        buf.copy_to_slice(&mut mempool_flow_len_bytes);
        let mut mempool_flow_len: u16 = u16::from_le_bytes(mempool_flow_len_bytes);
        while mempool_flow_len > 0 {
            let mut tx_hashes_bytes: [u8; 32] = [0; 32];
            buf.copy_to_slice(&mut tx_hashes_bytes);
            let tx_hash = tx_hashes_bytes.into();
            mempool_flow.push(tx_hash);

            mempool_flow_len -= 1;
        }

        Self {
            block_hash,
            block_timestamp,
            relay_timestamp,
            p2p_timestamp,
            proposer_fee_recipient,
            proposer_mev_reward,
            mempool_flow,
        }
    }
}

impl BincodeEncode for MetadataBincodeInner {
    fn encode<E: bincode::enc::Encoder>(
        &self,
        encoder: &mut E,
    ) -> Result<(), bincode::error::EncodeError> {
        let block_hash_bytes: [u8; 32] = self.block_hash.as_le_bytes().to_vec().try_into().unwrap();
        bincode::Encode::encode(&block_hash_bytes, encoder)?;

        bincode::Encode::encode(&self.block_timestamp, encoder)?;
        bincode::Encode::encode(&self.relay_timestamp, encoder)?;
        bincode::Encode::encode(&self.p2p_timestamp, encoder)?;

        let proposer_fee_recipient_bytes = self.proposer_fee_recipient.map(|addr| addr.0 .0);
        bincode::Encode::encode(&proposer_fee_recipient_bytes, encoder)?;

        bincode::Encode::encode(&self.proposer_mev_reward, encoder)?;

        let mempool_flow_bytes = self
            .mempool_flow
            .iter()
            .map(|flow| flow.0)
            .collect::<Vec<_>>();
        bincode::Encode::encode(&mempool_flow_bytes, encoder)?;

        Ok(())
    }
}

impl BincodeDecode for MetadataBincodeInner {
    fn decode<D: bincode::de::Decoder>(
        decoder: &mut D,
    ) -> Result<Self, bincode::error::DecodeError> {
        let block_hash_bytes: [u8; 32] = bincode::Decode::decode(decoder)?;
        let block_timestamp = bincode::Decode::decode(decoder)?;
        let relay_timestamp = bincode::Decode::decode(decoder)?;
        let p2p_timestamp = bincode::Decode::decode(decoder)?;
        let proposer_fee_recipient_bytes: Option<[u8; 20]> = bincode::Decode::decode(decoder)?;
        let proposer_mev_reward = bincode::Decode::decode(decoder)?;
        let mempool_flow_bytes: Vec<[u8; 32]> = bincode::Decode::decode(decoder)?;

        Ok(Self {
            block_hash: U256::from_le_bytes(block_hash_bytes),
            block_timestamp,
            relay_timestamp,
            p2p_timestamp,
            proposer_fee_recipient: proposer_fee_recipient_bytes.map(Into::into),
            proposer_mev_reward,
            mempool_flow: mempool_flow_bytes.into_iter().map(Into::into).collect(),
        })
    }
}

impl Encodable for MetadataBincodeInner {
    fn encode(&self, out: &mut dyn BufMut) {
        /*
                        let mut pre_compressed = Vec::new();
                self.block_hash.encode(&mut pre_compressed);
                self.block_timestamp.encode(&mut pre_compressed);
                self.relay_timestamp
                    .unwrap_or_default()
                    .encode(&mut pre_compressed);
                self.p2p_timestamp
                    .unwrap_or_default()
                    .encode(&mut pre_compressed);
                self.proposer_fee_recipient
                    .unwrap_or_default()
                    .encode(&mut pre_compressed);
                self.proposer_mev_reward
                    .unwrap_or_default()
                    .encode(&mut pre_compressed);
                self.mempool_flow.encode(&mut pre_compressed);


                //let serialized = serde_json::to_vec(&self).unwrap();

                //let encoded = encode_all(&serialized[..], 0).unwrap();

                let encoded = bincode::encode_to_vec(self, bincode::config::standard()).unwrap();
        */
        let encoded = EncodeMe::encode_me(self);

        out.put_slice(&encoded);
    }
}

impl Decodable for MetadataBincodeInner {
    fn decode(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        /*
               //let decompressed_buf = decode_all(buf).unwrap();


               let decompressed = &mut decompressed_buf.as_slice();

               let block_hash = U256::decode(decompressed)?;
               let block_timestamp = u64::decode(decompressed)?;
               let mut relay_timestamp = Some(u64::decode(decompressed)?);
               if relay_timestamp.as_ref().unwrap() == &0 {
                   relay_timestamp = None
               }
               let mut p2p_timestamp = Some(u64::decode(decompressed)?);
               if p2p_timestamp.as_ref().unwrap() == &0 {
                   p2p_timestamp = None
               }
               let mut proposer_fee_recipient = Some(Address::decode(decompressed)?);
               if proposer_fee_recipient.as_ref().unwrap().is_zero() {
                   proposer_fee_recipient = None
               }
               let mut proposer_mev_reward = Some(u128::decode(decompressed)?);
               if proposer_mev_reward.as_ref().unwrap() == &0 {
                   proposer_mev_reward = None
               }
               let mempool_flow = Vec::<TxHash>::decode(decompressed)?;

               Ok(Self {
                   block_hash,
                   block_timestamp,
                   relay_timestamp,
                   p2p_timestamp,
                   proposer_fee_recipient,
                   proposer_mev_reward,
                   mempool_flow,
               })


               //let decoded: Self = serde_json::from_slice(&decompressed_buf).unwrap();

               let len = <Self as Encodable>::length(&Self::default());
               let this_buf = buf.take(len).unwrap();

               let (decoded, len): (Self, usize) =
                   bincode::decode_from_slice(this_buf, bincode::config::standard()).unwrap();
        */
        let decoded = DecodeMe::decode_me(buf);

        Ok(decoded)
    }
}

impl Compress for MetadataBincodeInner {
    type Compressed = Vec<u8>;

    fn compress_to_buf<B: reth_primitives::bytes::BufMut + AsMut<[u8]>>(self, buf: &mut B) {
        let mut encoded = Vec::new();

        Encodable::encode(&self, &mut encoded);

        /*
                let block_hash =
            U256::from_str("0x10a27d25828e24f7b12257bbedda621a6d94f01a2f06fee4828d931027992283")
                .unwrap();
                if block_hash == self.block_hash {
                    println!("BINCODE COMPRESSED LEN: {}", encoded.len());
                }
        */
        buf.put_slice(&encoded);
    }
}

impl Decompress for MetadataBincodeInner {
    fn decompress<B: AsRef<[u8]>>(value: B) -> Result<Self, reth_db::DatabaseError> {
        let binding = value.as_ref().to_vec();
        let buf = &mut binding.as_slice();
        Decodable::decode(buf).map_err(|_| DatabaseError::Decode)
    }
}

impl From<MetadataBench> for MetadataBincodeData {
    fn from(value: MetadataBench) -> Self {
        MetadataBincodeData {
            block_number: value.block_number,
            inner:        MetadataBincodeInner {
                block_hash:             value.block_hash,
                block_timestamp:        value.block_timestamp,
                relay_timestamp:        value.relay_timestamp,
                p2p_timestamp:          value.p2p_timestamp,
                proposer_fee_recipient: value.proposer_fee_recipient,
                proposer_mev_reward:    value.proposer_mev_reward,
                mempool_flow:           value.mempool_flow,
            },
        }
    }
}
