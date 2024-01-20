use alloy_primitives::Address;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PoolsToAddresses(pub Vec<Address>);
