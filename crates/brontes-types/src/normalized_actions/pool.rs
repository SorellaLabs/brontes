use alloy_primitives::Address;
use serde::Deserialize;

use crate::Protocol;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct NormalizedNewPool {
    pub trace_index:  u64,
    pub protocol:     Protocol,
    pub pool_address: Address,
    pub tokens:       Vec<Address>,
}

impl TryFrom<NormalizedNewPool> for NormalizedPoolConfigUpdate {
    type Error = eyre::Report;

    fn try_from(value: NormalizedNewPool) -> Result<Self, Self::Error> {
        if value.tokens.len() < 2 {
            return Err(eyre::eyre!("normalized new pool doesn't have valid token entries"));
        }

        Ok(NormalizedPoolConfigUpdate {
            pool_address: value.pool_address,
            trace_index:  value.trace_index,
            protocol:     value.protocol,
            tokens:       value.tokens,
        })
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct NormalizedPoolConfigUpdate {
    pub trace_index:  u64,
    pub protocol:     Protocol,
    pub pool_address: Address,
    pub tokens:       Vec<Address>,
}
