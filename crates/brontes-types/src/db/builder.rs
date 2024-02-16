use alloy_primitives::Address;
use clickhouse::Row;
use redefined::Redefined;
use reth_rpc_types::beacon::BlsPublicKey;
use rkyv::{Archive, Deserialize as rDeserialize, Serialize as rSerialize};
use serde::{self, Deserialize, Serialize};

use crate::{
    db::redefined_types::primitives::{AddressRedefined, BlsPublicKeyRedefined},
    implement_table_value_codecs_with_zc,
    mev::MevBlock,
    serde_utils::{option_addresss, vec_address, vec_bls_pub_key},
};

#[derive(Debug, Default, Row, PartialEq, Clone, Eq, Serialize, Deserialize, Redefined)]
#[redefined_attr(derive(Debug, PartialEq, Clone, Serialize, rSerialize, rDeserialize, Archive))]
pub struct BuilderInfo {
    pub name: Option<String>,
    #[serde(with = "vec_bls_pub_key")]
    pub pub_keys: Vec<BlsPublicKey>,
    #[serde(with = "vec_address")]
    pub searchers_eoa: Vec<Address>,
    #[serde(with = "vec_address")]
    pub searchers_contract: Vec<Address>,
    #[serde(with = "option_addresss")]
    pub ultrasound_relay_collateral_address: Option<Address>,
}

implement_table_value_codecs_with_zc!(BuilderInfoRedefined);

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
