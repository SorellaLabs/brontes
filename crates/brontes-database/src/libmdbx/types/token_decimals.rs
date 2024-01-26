use alloy_primitives::Address;
use brontes_types::{db::token_info::TokenInfo, serde_utils::primitives::address_string};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use sorella_db_databases::clickhouse::{self, Row};

use super::{LibmdbxData, ReturnKV};
use crate::libmdbx::TokenDecimals;

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Clone, Row)]
pub struct TokenDecimalsData {
    #[serde(with = "address_string")]
    pub address: Address,
    pub info:    TokenInfo,
}

impl LibmdbxData<TokenDecimals> for TokenDecimalsData {
    fn into_key_val(&self) -> ReturnKV<TokenDecimals> {
        (self.address, self.info.clone()).into()
    }
}
