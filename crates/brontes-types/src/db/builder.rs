use alloy_primitives::Address;
use clickhouse::Row;
use redefined::Redefined;
use reth_rpc_types::beacon::BlsPublicKey;
use rkyv::{Archive, Deserialize as rDeserialize, Serialize as rSerialize};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;

use crate::{
    db::{
        redefined_types::primitives::{AddressRedefined, BlsPublicKeyRedefined},
        searcher::Fund,
    },
    implement_table_value_codecs_with_zc,
    mev::MevBlock,
    serde_utils::{addresss, option_addresss, option_fund, vec_address, vec_bls_pub_key},
    FastHashSet,
};

#[derive(Debug, Default, Row, PartialEq, Clone, Eq, Serialize, Deserialize, Redefined)]
#[redefined_attr(derive(Debug, PartialEq, Clone, Serialize, rSerialize, rDeserialize, Archive))]
pub struct BuilderInfo {
    pub name: Option<String>,
    #[redefined(same_fields)]
    #[serde(deserialize_with = "option_fund::deserialize")]
    #[serde(default)]
    pub fund: Option<Fund>,
    #[serde(with = "vec_bls_pub_key")]
    #[serde(default)]
    pub pub_keys: Vec<BlsPublicKey>,
    #[serde(with = "vec_address")]
    #[serde(default)]
    pub searchers_eoas: Vec<Address>,
    #[serde(with = "vec_address")]
    #[serde(default)]
    pub searchers_contracts: Vec<Address>,
    #[serde(with = "option_addresss")]
    #[serde(default)]
    pub ultrasound_relay_collateral_address: Option<Address>,
}

impl BuilderInfo {
    pub fn merge(&mut self, other: BuilderInfo) {
        self.name = other.name.or(self.name.take());
        self.fund = other.fund.or(self.fund.take());

        self.pub_keys = self
            .pub_keys
            .iter()
            .chain(other.pub_keys.iter())
            .cloned()
            .collect::<FastHashSet<_>>()
            .into_iter()
            .collect();

        self.searchers_eoas = self
            .searchers_eoas
            .iter()
            .chain(other.searchers_eoas.iter())
            .cloned()
            .collect::<FastHashSet<_>>()
            .into_iter()
            .collect();

        self.searchers_contracts = self
            .searchers_contracts
            .iter()
            .chain(other.searchers_contracts.iter())
            .cloned()
            .collect::<FastHashSet<_>>()
            .into_iter()
            .collect();

        self.ultrasound_relay_collateral_address = other
            .ultrasound_relay_collateral_address
            .or(self.ultrasound_relay_collateral_address.take());
    }

    pub fn describe(&self) -> String {
        let mut description = String::new();

        if let Some(name) = &self.name {
            description.push_str(name);
        } else {
            // If no name is provided, use a placeholder or generic term
            description.push_str("Unknown Block Builder");
        }

        // Add fund information if it exists and is not Fund::None
        if let Some(fund) = &self.fund {
            if *fund != Fund::None {
                description.push_str(" operated by ");
                description.push_str(&fund.to_string());
            }
        }

        description
    }
}

implement_table_value_codecs_with_zc!(BuilderInfoRedefined);

#[serde_as]
#[derive(Debug, Default, Row, PartialEq, Clone, Serialize, Deserialize)]
pub struct BuilderInfoWithAddress {
    #[serde(with = "addresss")]
    pub address: Address,
    pub name: Option<String>,
    #[serde(deserialize_with = "option_fund::deserialize")]
    pub fund: Option<Fund>,
    #[serde(with = "vec_bls_pub_key")]
    pub pub_keys: Vec<BlsPublicKey>,
    #[serde(with = "vec_address")]
    pub searchers_eoas: Vec<Address>,
    #[serde(with = "vec_address")]
    pub searchers_contracts: Vec<Address>,
    #[serde(with = "option_addresss")]
    pub ultrasound_relay_collateral_address: Option<Address>,
}

impl BuilderInfoWithAddress {
    pub fn new_with_address(address: Address, info: BuilderInfo) -> Self {
        Self {
            address,
            name: info.name,
            fund: info.fund,
            pub_keys: info.pub_keys,
            searchers_eoas: info.searchers_eoas,
            searchers_contracts: info.searchers_contracts,
            ultrasound_relay_collateral_address: info.ultrasound_relay_collateral_address,
        }
    }
}

#[derive(Debug, Default, Row, PartialEq, Clone, Serialize, Deserialize, Redefined)]
#[redefined_attr(derive(Debug, PartialEq, Clone, Serialize, rSerialize, rDeserialize, Archive))]
pub struct BuilderStats {
    pub pnl: f64,
    pub blocks_built: u64,
    pub last_active: u64,
}

implement_table_value_codecs_with_zc!(BuilderStatsRedefined);

impl BuilderStats {
    pub fn update_with_block(&mut self, block: &MevBlock) {
        self.pnl += block.builder_profit_usd + block.builder_mev_profit_usd;
        self.blocks_built += 1;
        self.last_active = block.block_number;
    }
}

#[derive(Debug, Default, Row, PartialEq, Clone, Serialize, Deserialize)]
pub struct BuilderStatsWithAddress {
    #[serde(with = "addresss")]
    pub address: Address,
    pub pnl: f64,
    pub blocks_built: u64,
    pub last_active: u64,
}

impl BuilderStatsWithAddress {
    pub fn new_with_address(address: Address, stats: BuilderStats) -> Self {
        Self {
            address,
            pnl: stats.pnl,
            blocks_built: stats.blocks_built,
            last_active: stats.last_active,
        }
    }
}
