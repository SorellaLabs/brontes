use alloy_primitives::Address;
use alloy_rlp::{RlpDecodable, RlpEncodable};
use serde::{Deserialize, Serialize};
use sorella_db_databases::{clickhouse, Row};

use super::serde::address_string;

#[derive(Debug, Row, Clone, PartialEq, Eq, Serialize, Deserialize, RlpDecodable, RlpEncodable)]
pub struct DexQuoteLibmdbx {
    #[serde(with = "address_string")]
    pub token0:             Address,
    #[serde(with = "address_string")]
    pub token1:             Address,
    pub pool_keys_for_pair: Vec<PoolKeysForPairLibmdbx>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, RlpDecodable, RlpEncodable)]
pub struct PoolKeysForPairLibmdbx {
    #[serde(with = "address_string")]
    pub pool:          Address,
    pub run:           u64,
    pub batch:         u64,
    pub update_nonce:  u16,
    #[serde(with = "address_string")]
    pub pool_key_base: Address,
}
