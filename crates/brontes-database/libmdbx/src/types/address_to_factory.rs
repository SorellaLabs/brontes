use alloy_primitives::Address;
use alloy_rlp::{ RlpDecodable, RlpEncodable};
use brontes_types::exchanges::StaticBindingsDb;
use serde::{Deserialize, Serialize};
use sorella_db_databases::{clickhouse, Row};

use super::{LibmdbxData};
use crate::AddressToFactory;

#[derive(Debug, Serialize, Deserialize, Clone, Row, RlpDecodable, RlpEncodable)]
pub struct AddressToFactoryData {
    pub address:      Address,
    pub factory_type: StaticBindingsDb,
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
