mod relays;
use std::str::FromStr;

pub use relays::*;

mod bids_payloads;
pub use bids_payloads::*;
use reth_primitives::Address;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelayBlockMetadata {
    pub block_number:           u64,
    pub relay_timestamp:        Option<u64>,
    pub proposer_fee_recipient: Address,
    pub proposer_mev_reward:    u128,
}

impl TryFrom<RelayBid> for RelayBlockMetadata {
    type Error = eyre::ErrReport;

    fn try_from(value: RelayBid) -> eyre::Result<Self> {
        Ok(Self {
            block_number:           value.block_number,
            relay_timestamp:        Some(value.timestamp_ms),
            proposer_fee_recipient: Address::from_str(&value.proposer_fee_recipient)?,
            proposer_mev_reward:    value.value,
        })
    }
}

impl TryFrom<RelayPayload> for RelayBlockMetadata {
    type Error = eyre::ErrReport;

    fn try_from(value: RelayPayload) -> eyre::Result<Self> {
        Ok(Self {
            block_number:           value.block_number,
            relay_timestamp:        None,
            proposer_fee_recipient: Address::from_str(&value.proposer_fee_recipient)?,
            proposer_mev_reward:    value.value,
        })
    }
}
