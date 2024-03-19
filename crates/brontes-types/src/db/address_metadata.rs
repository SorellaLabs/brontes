use alloy_primitives::Address;
use clickhouse::Row;
use redefined::{self_convert_redefined, Redefined};
use rkyv::{Archive, Deserialize as rDeserialize, Serialize as rSerialize};
use serde::{Deserialize, Serialize};

use crate::{
    db::redefined_types::primitives::AddressRedefined,
    implement_table_value_codecs_with_zc,
    serde_utils::{option_contract_info, socials},
};

#[derive(Debug, Default, Row, PartialEq, Clone, Eq, Serialize, Deserialize, Redefined)]
#[redefined_attr(derive(Debug, PartialEq, Clone, Serialize, rSerialize, rDeserialize, Archive))]
pub struct AddressMetadata {
    pub entity_name:     Option<String>,
    pub nametag:         Option<String>,
    pub labels:          Vec<String>,
    #[serde(rename = "type")]
    pub address_type:    Option<String>,
    #[serde(deserialize_with = "option_contract_info::deserialize")]
    #[cfg_attr(api, serde(serialize_with = "option_contract_info::Serialize"))]
    pub contract_info:   Option<ContractInfo>,
    pub ens:             Option<String>,
    #[serde(deserialize_with = "socials::deserialize")]
    #[cfg_attr(api, serde(serialize_with = "socials::Serialize"))]
    #[redefined(same_fields)]
    pub social_metadata: Socials,
}

impl AddressMetadata {
    pub fn is_verified(&self) -> bool {
        self.contract_info
            .as_ref()
            .map_or(false, |c| c.verified_contract.unwrap_or(false))
    }

    pub fn describe(&self) -> Option<String> {
        self.entity_name
            .clone()
            .or_else(|| self.nametag.clone())
            .or_else(|| self.address_type.clone())
            .or_else(|| self.ens.clone())
            .or_else(|| self.social_metadata.twitter.clone())
            .or_else(|| self.labels.first().cloned())
    }

    pub fn get_contract_type(&self) -> ContractType {
        if self.is_cex_exchange() {
            return ContractType::CexExchange;
        }

        if self.is_cex() {
            return ContractType::Cex;
        }

        if self.is_aggregator() {
            return ContractType::Router;
        }

        if let Some(contract_type) = self.get_contract_type_from_nametag() {
            return contract_type;
        }

        self.get_contract_type_from_labels()
            .unwrap_or(ContractType::Unknown)
    }

    fn is_cex(&self) -> bool {
        self.address_type
            .as_deref()
            .map_or(false, |t| t.eq_ignore_ascii_case("cex"))
    }

    fn is_aggregator(&self) -> bool {
        self.address_type
            .as_deref()
            .map_or(false, |t| t.eq_ignore_ascii_case("aggregator"))
    }

    fn is_cex_exchange(&self) -> bool {
        self.labels
            .iter()
            .any(|label| label.to_lowercase().contains("exchange"))
            && self.is_cex()
    }

    fn get_contract_type_from_nametag(&self) -> Option<ContractType> {
        self.nametag.as_ref().and_then(|nametag| {
            let nametag_lower = nametag.to_lowercase();
            match nametag_lower.as_str() {
                n if n.starts_with("mev bot:") => Some(ContractType::MevBot),
                n if n.contains("router") => Some(ContractType::Router),
                n if n.contains("protocol") => Some(ContractType::Protocol),
                n if n.contains("exchange") => Some(ContractType::Exchange),
                n if n.contains("bridge") => Some(ContractType::Bridge),
                _ => None,
            }
        })
    }

    fn get_contract_type_from_labels(&self) -> Option<ContractType> {
        self.labels.iter().find_map(|label| {
            let label_lower = label.to_lowercase();
            match label_lower.as_str() {
                l if l.contains("mev bot") => Some(ContractType::MevBot),
                l if l.contains("router") => Some(ContractType::Router),
                l if l.contains("protocol") => Some(ContractType::Protocol),
                l if l.contains("exchange") => Some(ContractType::Exchange),
                _ => None,
            }
        })
    }
}

#[derive(Debug, Clone)]
pub enum ContractType {
    MevBot,
    Router,
    Exchange,
    Cex,
    Protocol,
    CexExchange,
    Bridge,
    Unknown,
}

impl ContractType {
    pub fn could_be_mev_contract(&self) -> bool {
        matches!(self, ContractType::MevBot | ContractType::Unknown)
    }
}

implement_table_value_codecs_with_zc!(AddressMetadataRedefined);

#[derive(Debug, Default, PartialEq, Clone, Eq, Serialize, Deserialize, Redefined)]
#[redefined_attr(derive(Debug, PartialEq, Clone, Serialize, rSerialize, rDeserialize, Archive))]
pub struct ContractInfo {
    pub verified_contract: Option<bool>,
    pub contract_creator:  Option<Address>,
    pub reputation:        Option<u8>,
}

#[derive(
    Debug, Default, PartialEq, Clone, Eq, Serialize, Deserialize, rSerialize, rDeserialize, Archive,
)]
pub struct Socials {
    pub twitter:           Option<String>,
    pub twitter_followers: Option<u64>,
    pub website_url:       Option<String>,
    pub crunchbase:        Option<String>,
    pub linkedin:          Option<String>,
}

self_convert_redefined!(Socials);
