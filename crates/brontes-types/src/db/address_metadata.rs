use std::str::FromStr;

use alloy_primitives::Address;
use clickhouse::Row;
use redefined::{self_convert_redefined, Redefined};
use rkyv::{Archive, Deserialize as rDeserialize, Serialize as rSerialize};
use serde::{Deserialize, Serialize};

use crate::{
    db::redefined_types::primitives::AddressRedefined, implement_table_value_codecs_with_zc,
};

#[derive(Debug, Default, Row, PartialEq, Clone, Eq, Serialize, Deserialize, Redefined)]
#[redefined_attr(derive(Debug, PartialEq, Clone, Serialize, rSerialize, rDeserialize, Archive))]
pub struct AddressMetadata {
    pub entity_name:     Option<String>,
    pub nametag:         Option<String>,
    pub labels:          Vec<String>,
    #[serde(rename = "type")]
    pub address_type:    Option<String>,
    pub contract_info:   Option<ContractInfo>,
    pub ens:             Option<String>,
    #[redefined(same_fields)]
    pub social_metadata: Socials,
}

impl AddressMetadata {
    pub fn is_verified(&self) -> bool {
        self.contract_info
            .as_ref()
            .map_or(false, |c| c.verified_contract.unwrap_or(false))
    }
}

implement_table_value_codecs_with_zc!(AddressMetadataRedefined);

#[derive(Debug, Default, PartialEq, Clone, Eq, Redefined)]
#[redefined_attr(derive(Debug, PartialEq, Clone, Serialize, rSerialize, rDeserialize, Archive))]
pub struct ContractInfo {
    pub verified_contract: Option<bool>,
    pub contract_creator:  Option<Address>,
    pub reputation:        Option<u8>,
}

impl<'de> Deserialize<'de> for ContractInfo {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let (verified_contract, contract_creator_opt, reputation): (
            Option<bool>,
            Option<String>,
            Option<u8>,
        ) = Deserialize::deserialize(deserializer)?;

        Ok(ContractInfo {
            verified_contract,
            contract_creator: contract_creator_opt
                .map(|f| Address::from_str(&f).ok())
                .flatten(),
            reputation,
        })
    }
}

impl Serialize for ContractInfo {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let ser = (self.verified_contract, self.contract_creator, self.reputation);
        ser.serialize(serializer)
    }
}

#[derive(Debug, Default, PartialEq, Clone, Eq, rSerialize, rDeserialize, Archive)]
pub struct Socials {
    pub twitter:           Option<String>,
    pub twitter_followers: Option<u64>,
    pub website_url:       Option<String>,
    pub crunchbase:        Option<String>,
    pub linkedin:          Option<String>,
}

impl Serialize for Socials {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serde::Serialize::serialize(
            &(
                self.twitter,
                self.twitter_followers,
                self.website_url,
                self.crunchbase,
                self.linkedin,
            ),
            serializer,
        )
    }
}

type SocalDecode = (Option<String>, Option<u64>, Option<String>, Option<String>, Option<String>);

impl<'de> Deserialize<'de> for Socials {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let (twitter, twitter_followers, website_url, crunchbase, linkedin): SocalDecode =
            Deserialize::deserialize(deserializer)?;

        Ok(Socials { twitter, twitter_followers, website_url, crunchbase, linkedin }.into())
    }
}

self_convert_redefined!(Socials);
