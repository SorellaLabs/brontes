use std::str::FromStr;

use alloy_primitives::Address;
use redefined::Redefined;
use rkyv::{Archive, Deserialize as rDeserialize, Serialize as rSerialize};
use serde::{self, Deserialize, Serialize};
use sorella_db_databases::{clickhouse, clickhouse::Row};

use crate::{
    db::redefined_types::primitives::AddressRedefined,
    implement_table_value_codecs_with_zc,
    serde_utils::{addresss, option_addresss, static_bindings},
    Protocol,
};

#[derive(Debug, Default, Row, PartialEq, Clone, Eq, Serialize, Deserialize, Redefined, Hash)]
#[redefined_attr(derive(
    Debug,
    PartialEq,
    Clone,
    Serialize,
    rSerialize,
    rDeserialize,
    Archive,
    Hash
))]
pub struct ProtocolInfo {
    #[serde(with = "static_bindings")]
    #[redefined(same_fields)]
    pub protocol: Protocol,
    #[serde(with = "addresss")]
    pub token0: Address,
    #[serde(with = "addresss")]
    pub token1: Address,
    #[serde(with = "option_addresss")]
    pub token2: Option<Address>,
    #[serde(with = "option_addresss")]
    pub token3: Option<Address>,
    #[serde(with = "option_addresss")]
    pub token4: Option<Address>,
    #[serde(with = "option_addresss")]
    pub curve_lp_token: Option<Address>,
    pub init_block: u64,
}

impl IntoIterator for ProtocolInfo {
    type IntoIter = std::vec::IntoIter<Self::Item>;
    type Item = Address;

    fn into_iter(self) -> Self::IntoIter {
        vec![
            Some(self.token0),
            Some(self.token1),
            self.token2,
            self.token3,
            self.token4,
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .into_iter()
    }
}

impl From<(Vec<String>, u64, String)> for ProtocolInfo {
    fn from(value: (Vec<String>, u64, String)) -> Self {
        let init_block = value.1;
        let protocol = Protocol::parse_string(value.2);
        let value = value.0;
        let mut iter = value.into_iter();
        ProtocolInfo {
            protocol,
            token0: Address::from_str(&iter.next().unwrap()).unwrap(),
            token1: Address::from_str(&iter.next().unwrap()).unwrap(),
            token2: iter.next().and_then(|a| Address::from_str(&a).ok()),
            token3: iter.next().and_then(|a| Address::from_str(&a).ok()),
            token4: iter.next().and_then(|a| Address::from_str(&a).ok()),
            curve_lp_token: iter.next().and_then(|a| Address::from_str(&a).ok()),
            init_block,
        }
    }
}

implement_table_value_codecs_with_zc!(ProtocolInfoRedefined);
