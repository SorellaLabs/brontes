use alloy_primitives::Address;
use brontes_pricing::Protocol;
use serde::{Deserialize, Serialize};
use sorella_db_databases::clickhouse::{self, Row};

use super::LibmdbxData;
use crate::libmdbx::AddressToFactory;

/// rlp encoding for libmdbx here is fine since it is just an enum
#[derive(Debug, Serialize, Deserialize, Clone, Row)]
pub struct AddressToFactoryData {
    pub address:      Address,
    pub factory_type: Protocol,
}

impl AddressToFactoryData {
    pub fn new(address: Address, factory_type: Protocol) -> Self {
        Self { factory_type, address }
    }
}

impl LibmdbxData<AddressToFactory> for AddressToFactoryData {
    fn into_key_val(
        &self,
    ) -> (
        <AddressToFactory as reth_db::table::Table>::Key,
        <AddressToFactory as reth_db::table::Table>::Value,
    ) {
        (self.address, self.factory_type)
    }
}
