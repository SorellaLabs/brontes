use std::str::FromStr;

use alloy_primitives::Address;
use clickhouse::{fixed_string::FixedString, Row};
use redefined::Redefined;
use rkyv::{Archive, Deserialize as rDeserialize, Serialize as rSerialize};
use serde::{Deserialize, Serialize};

use crate::{
    db::redefined_types::primitives::AddressRedefined,
    implement_table_value_codecs_with_zc,
    serde_utils::{addresss, option_addresss, protocol},
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
    #[serde(with = "protocol")]
    #[redefined(same_fields)]
    pub protocol:       Protocol,
    #[serde(with = "addresss")]
    pub token0:         Address,
    #[serde(with = "addresss")]
    pub token1:         Address,
    #[serde(with = "option_addresss")]
    pub token2:         Option<Address>,
    #[serde(with = "option_addresss")]
    pub token3:         Option<Address>,
    #[serde(with = "option_addresss")]
    pub token4:         Option<Address>,
    #[serde(with = "option_addresss")]
    pub curve_lp_token: Option<Address>,
    pub init_block:     u64,
}

impl ProtocolInfo {
    pub fn get_tokens(&self) -> Vec<Address> {
        let mut tokens = vec![self.token0, self.token1]
            .into_iter()
            .filter(|token| *token != Address::default())
            .collect::<Vec<_>>();

        tokens.extend(
            [self.token2, self.token3, self.token4, self.curve_lp_token]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>(),
        );

        tokens
    }
}

impl IntoIterator for ProtocolInfo {
    type IntoIter = std::vec::IntoIter<Self::Item>;
    type Item = Address;

    fn into_iter(self) -> Self::IntoIter {
        vec![Some(self.token0), Some(self.token1), self.token2, self.token3, self.token4]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .into_iter()
    }
}

impl From<(Vec<String>, u64, String, Option<String>)> for ProtocolInfo {
    fn from(value: (Vec<String>, u64, String, Option<String>)) -> Self {
        let init_block = value.1;
        let protocol = Protocol::from_db_string(&value.2);
        let curve_lp_token = value.3.map(|s| Address::from_str(&s).unwrap());
        let value = value.0;
        let mut iter = value.into_iter();
        ProtocolInfo {
            protocol,
            token0: Address::from_str(&iter.next().unwrap()).unwrap(),
            token1: Address::from_str(&iter.next().unwrap()).unwrap(),
            token2: iter.next().and_then(|a| Address::from_str(&a).ok()),
            token3: iter.next().and_then(|a| Address::from_str(&a).ok()),
            token4: iter.next().and_then(|a| Address::from_str(&a).ok()),
            curve_lp_token,
            init_block,
        }
    }
}

implement_table_value_codecs_with_zc!(ProtocolInfoRedefined);

#[derive(Debug, Default, Row, PartialEq, Clone, Eq, Serialize, Deserialize)]
pub struct ProtocolInfoClickhouse {
    pub protocol:         String,
    pub protocol_subtype: String,
    pub address:          FixedString,
    pub tokens:           Vec<FixedString>,
    pub curve_lp_token:   Option<FixedString>,
    pub init_block:       u64,
}

impl ProtocolInfoClickhouse {
    pub fn new(
        block: u64,
        address: Address,
        tokens: &[Address],
        curve_lp_token: Option<Address>,
        classifier_name: Protocol,
    ) -> Self {
        let (protocol, protocol_subtype) = classifier_name.into_clickhouse_protocol();
        Self {
            protocol:         protocol.to_string(),
            protocol_subtype: protocol_subtype.to_string(),
            address:          format!("{:?}", address).into(),
            tokens:           tokens.iter().map(|t| format!("{:?}", t).into()).collect(),
            curve_lp_token:   curve_lp_token.map(|t| format!("{:?}", t).into()),
            init_block:       block,
        }
    }
}
