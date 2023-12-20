use alloy_rlp::{Decodable, Encodable};
pub use brontes_types::exchanges::StaticBindingsDb;
use brontes_types::libmdbx::serde::address_string;
use reth_db::{
    table::{Compress, Decompress},
    DatabaseError,
};
use reth_primitives::{Address, BufMut};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use sorella_db_databases::{clickhouse, Row};

use crate::{tables::AddressToProtocol, types::utils::static_bindings, LibmdbxData};

#[serde_as]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Row)]
pub struct AddressToProtocolData {
    #[serde(with = "address_string")]
    pub address: Address,

    #[serde(with = "static_bindings")]
    pub classifier_name: StaticBindingsDb,
}

impl LibmdbxData<AddressToProtocol> for AddressToProtocolData {
    fn into_key_val(
        &self,
    ) -> (
        <AddressToProtocol as reth_db::table::Table>::Key,
        <AddressToProtocol as reth_db::table::Table>::Value,
    ) {
        (self.address, self.classifier_name.clone())
    }
}
