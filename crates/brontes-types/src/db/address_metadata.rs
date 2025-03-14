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
    pub contract_info:   Option<ContractInfo>,
    pub ens:             Option<String>,
    #[serde(deserialize_with = "socials::deserialize")]
    #[serde(serialize_with = "socials::serialize")]
    #[redefined(same_fields)]
    pub social_metadata: Socials,
}

impl AddressMetadata {
    pub fn is_verified(&self) -> bool {
        self.contract_info
            .as_ref().is_some_and(|c| c.verified_contract.unwrap_or(false))
    }

    pub fn describe(&self) -> Option<String> {
        self.nametag
            .clone()
            .or_else(|| self.entity_name.clone())
            .or_else(|| self.address_type.clone())
            .or_else(|| self.ens.clone())
            .or_else(|| self.social_metadata.twitter.clone())
            .or_else(|| self.labels.first().cloned())
    }

    pub fn get_contract_type(&self) -> ContractType {
        if self.is_cex_exchange() {
            return ContractType::CexExchange;
        }

        if self.is_settlement_contract() {
            return ContractType::SolverSettlement;
        }

        if self.is_automation_contract() {
            return ContractType::DefiAutomation;
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

    fn is_automation_contract(&self) -> bool {
        self.labels
            .iter()
            .any(|label| label.to_lowercase().contains("automation"))
    }

    fn is_settlement_contract(&self) -> bool {
        if let Some(nametag) = &self.nametag {
            if nametag.eq_ignore_ascii_case("UniswapX")
                || nametag.eq_ignore_ascii_case("CoW Protocol")
            {
                return true;
            }
        }

        self.labels
            .iter()
            .any(|label| label.eq_ignore_ascii_case("settlement"))
    }

    fn is_cex(&self) -> bool {
        self.address_type
            .as_deref().is_some_and(|t| t.eq_ignore_ascii_case("cex"))
    }

    fn is_aggregator(&self) -> bool {
        self.address_type
            .as_deref().is_some_and(|t| t.eq_ignore_ascii_case("aggregator"))
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

    pub fn merge(&mut self, other: Self) {
        if other.entity_name.is_some() {
            self.entity_name = other.entity_name;
        }

        for label in other.labels.into_iter() {
            if !self.labels.iter().any(|l| l.eq_ignore_ascii_case(&label)) {
                self.labels.push(label);
            }
        }

        if other.nametag.is_some() {
            self.nametag = other.nametag
        }

        if other.address_type.is_some() {
            self.address_type = other.address_type
        }

        if other.ens.is_some() {
            self.ens = other.ens
        }

        if let Some(other_contract_info) = other.contract_info {
            match &mut self.contract_info {
                Some(c) => c.merge(other_contract_info),
                None => self.contract_info = Some(other_contract_info),
            }
        }

        self.social_metadata.merge(other.social_metadata);
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
    SolverSettlement,
    DefiAutomation,
    Unknown,
}

impl ContractType {
    pub fn could_be_mev_contract(&self) -> bool {
        matches!(self, ContractType::MevBot | ContractType::Unknown)
    }

    pub fn is_solver_settlement(&self) -> bool {
        matches!(self, ContractType::SolverSettlement)
    }

    pub fn is_mev_contract(&self) -> bool {
        matches!(self, ContractType::MevBot)
    }

    pub fn is_defi_automation(&self) -> bool {
        matches!(self, ContractType::DefiAutomation)
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

impl ContractInfo {
    fn merge(&mut self, other: ContractInfo) {
        if let Some(verified_contract) = other.verified_contract {
            self.verified_contract = Some(verified_contract);
        }

        if let Some(contract_creator) = other.contract_creator {
            self.contract_creator = Some(contract_creator);
        }

        if let Some(reputation) = other.reputation {
            self.reputation = Some(reputation);
        }
    }
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

impl Socials {
    fn merge(&mut self, other: Socials) {
        if let Some(twitter) = other.twitter {
            self.twitter = Some(twitter);
        }
        if let Some(twitter_followers) = other.twitter_followers {
            self.twitter_followers = Some(twitter_followers);
        }
        if let Some(website_url) = other.website_url {
            self.website_url = Some(website_url);
        }
        if let Some(crunchbase) = other.crunchbase {
            self.crunchbase = Some(crunchbase);
        }
        if let Some(linkedin) = other.linkedin {
            self.linkedin = Some(linkedin);
        }
    }
}

self_convert_redefined!(Socials);
