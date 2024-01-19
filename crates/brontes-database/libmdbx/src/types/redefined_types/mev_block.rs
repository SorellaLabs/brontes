use alloy_rlp::{Decodable, Encodable};
use brontes_types::{
    classified_mev::{ClassifiedMev, MevBlock, MevType, SpecificMev},
    libmdbx::redefined_types::{
        malachite::Redefined_Rational,
        primitives::{Redefined_Address, Redefined_FixedBytes},
    },
};
use bytes::BufMut;
use redefined::{Redefined, RedefinedConvert};
use reth_db::{
    table::{Compress, Decompress},
    DatabaseError,
};
use rkyv::Deserialize;

use super::price_maps::Redefined_Pair;
use crate::types::{
    dex_price::{DexQuote, DexQuoteWithIndex},
    mev_block::MevBlockWithClassified,
};

/*
#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, Redefined)]
#[redefined(MevBlockWithClassified)]
pub struct Redefined_MevBlockWithClassified {
    pub block: Redefined_MevBlock,
    pub mev:   Vec<(Redefined_ClassifiedMev, Box<dyn SpecificMev>)>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, Redefined)]
#[redefined(MevBlock)]
pub struct Redefined_MevBlock {
    pub block_hash: Redefined_FixedBytes<32>,
    pub block_number: u64,
    pub mev_count: u64,
    pub finalized_eth_price: f64,
    pub cumulative_gas_used: u128,
    pub cumulative_gas_paid: u128,
    pub total_bribe: u128,
    pub cumulative_mev_priority_fee_paid: u128,
    pub builder_address: Redefined_Address,
    pub builder_eth_profit: i128,
    pub builder_finalized_profit_usd: f64,
    pub proposer_fee_recipient: Option<Redefined_Address>,
    pub proposer_mev_reward: Option<u128>,
    pub proposer_finalized_profit_usd: Option<f64>,
    pub cumulative_mev_finalized_profit_usd: f64,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, Clone, Redefined)]
#[redefined(ClassifiedMev)]
pub struct Redefined_ClassifiedMev {
    pub block_number:         u64,
    pub tx_hash:              Redefined_FixedBytes<32>,
    pub eoa:                  Redefined_Address,
    pub mev_contract:         Redefined_Address,
    pub mev_profit_collector: Vec<Redefined_Address>,
    pub finalized_profit_usd: f64,
    pub finalized_bribe_usd:  f64,
    pub mev_type:             MevType,
}
*/
