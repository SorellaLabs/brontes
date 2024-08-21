mod relays;
pub use relays::*;

mod bids_payloads;
pub use bids_payloads::*;

pub struct RelayBlockMetadata {
    pub block_number:           u64,
    pub relay_timestamp:        Option<u64>,
    pub proposer_fee_recipient: String,
    pub proposer_mev_reward:    u128,
}

impl From<RelayBid> for RelayBlockMetadata {
    fn from(value: RelayBid) -> Self {
        Self {
            block_number:           value.block_number,
            relay_timestamp:        Some(value.timestamp_ms),
            proposer_fee_recipient: value.proposer_fee_recipient,
            proposer_mev_reward:    value.value,
        }
    }
}

impl From<RelayPayload> for RelayBlockMetadata {
    fn from(value: RelayPayload) -> Self {
        Self {
            block_number:           value.block_number,
            relay_timestamp:        None,
            proposer_fee_recipient: value.proposer_fee_recipient,
            proposer_mev_reward:    value.value,
        }
    }
}
