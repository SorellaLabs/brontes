use std::hash::Hash;

use ::relays_openapi::models::{
    GetDeliveredPayloads200ResponseInner, GetReceivedBids200ResponseInner,
};
use serde::{Deserialize, Serialize};

use super::Relays;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct RelayBid {
    pub relay:                  Relays,
    pub slot:                   u64,
    pub parent_hash:            String,
    pub block_hash:             String,
    pub builder_pubkey:         String,
    pub proposer_fee_recipient: String,
    pub gas_limit:              u64,
    pub gas_used:               u64,
    pub value:                  u128,
    pub block_number:           u64,
    pub num_tx:                 u64,
    pub timestamp:              u64,
    pub timestamp_ms:           u64,
}

impl RelayBid {
    pub fn new(bid: GetReceivedBids200ResponseInner, relay: Relays) -> Self {
        Self {
            relay,
            slot: bid.slot.unwrap().parse().unwrap(),
            parent_hash: bid.parent_hash.unwrap(),
            block_hash: bid.block_hash.unwrap(),
            builder_pubkey: bid.builder_pubkey.unwrap(),
            proposer_fee_recipient: bid.proposer_fee_recipient.unwrap(),
            gas_limit: bid.gas_limit.unwrap().parse().unwrap(),
            gas_used: bid.gas_used.unwrap().parse().unwrap(),
            value: bid.value.unwrap().parse().unwrap(),
            block_number: bid.block_number.unwrap().parse().unwrap(),
            num_tx: bid.num_tx.unwrap().parse().unwrap(),
            timestamp: bid.timestamp.unwrap().parse().unwrap(),
            timestamp_ms: bid.timestamp_ms.unwrap().parse().unwrap(),
        }
    }

    pub fn calculate_epoch(&self) -> u64 {
        self.slot / 32
    }
}

impl PartialOrd for RelayBid {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for RelayBid {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.timestamp_ms.cmp(&other.timestamp_ms)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Hash, PartialOrd)]
pub struct RelayPayload {
    pub relay:                  Relays,
    pub slot:                   u64,
    pub parent_hash:            String,
    pub block_hash:             String,
    pub builder_pubkey:         String,
    pub proposer_fee_recipient: String,
    pub gas_limit:              u64,
    pub gas_used:               u64,
    pub value:                  u128,
    pub block_number:           u64,
    pub num_tx:                 u64,
}

impl RelayPayload {
    pub fn new(payload: GetDeliveredPayloads200ResponseInner, relay: Relays) -> Self {
        Self {
            relay,
            slot: payload.slot.unwrap().parse().unwrap(),
            parent_hash: payload.parent_hash.unwrap(),
            block_hash: payload.block_hash.unwrap(),
            builder_pubkey: payload.builder_pubkey.unwrap(),
            proposer_fee_recipient: payload.proposer_fee_recipient.unwrap(),
            gas_limit: payload.gas_limit.unwrap().parse().unwrap(),
            gas_used: payload.gas_used.unwrap().parse().unwrap(),
            value: payload.value.unwrap().parse().unwrap(),
            block_number: payload.block_number.unwrap().parse().unwrap(),
            num_tx: payload.num_tx.unwrap().parse().unwrap(),
        }
    }

    pub fn calculate_epoch(&self) -> u64 {
        self.slot / 32
    }
}
