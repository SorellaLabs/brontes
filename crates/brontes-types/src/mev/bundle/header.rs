use std::fmt::Debug;

use alloy_primitives::Address;
use reth_primitives::B256;
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use sorella_db_databases::{
    clickhouse,
    clickhouse::{fixed_string::FixedString, Row},
};

use super::MevType;
#[allow(unused_imports)]
use crate::{
    display::utils::{display_sandwich, print_mev_type_header},
    normalized_actions::{NormalizedBurn, NormalizedLiquidation, NormalizedMint, NormalizedSwap},
    serde_primitives::vec_fixed_string,
    GasDetails,
};

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Row, Clone, Default)]
pub struct BundleHeader {
    pub block_number:         u64,
    pub tx_index:             u64,
    #[serde_as(as = "FixedString")]
    // For a sandwich this is always the first frontrun tx hash
    pub tx_hash: B256,
    #[serde_as(as = "FixedString")]
    pub eoa:                  Address,
    #[serde_as(as = "FixedString")]
    pub mev_contract:         Address,
    #[serde(with = "vec_fixed_string")]
    pub mev_profit_collector: Vec<Address>,
    pub profit_usd:           f64,
    pub token_profits:        TokenProfits,
    pub bribe_usd:            f64,
    pub mev_type:             MevType,
}

#[serde_as]
#[derive(Debug, Deserialize, Row, Clone, Default, Serialize)]
pub struct TokenProfit {
    pub profit_collector: Address,
    pub token:            Address,
    pub amount:           f64,
    pub usd_value:        f64,
}

#[serde_as]
#[derive(Debug, Deserialize, Row, Clone, Default, Serialize)]
pub struct TokenProfits {
    pub profits: Vec<TokenProfit>,
}

impl TokenProfits {
    //TODO: Find is short circuiting, in this case this should be fine but not
    // entirely sure.
    pub fn compose(&mut self, to_compose: &TokenProfits) {
        for profit in &to_compose.profits {
            if let Some(existing_profit) = self
                .profits
                .iter_mut()
                .find(|p| p.profit_collector == profit.profit_collector && p.token == profit.token)
            {
                if existing_profit.amount < profit.amount {
                    existing_profit.amount = profit.amount;
                }
            }
        }
    }
    //TODO: Alternatively we could do something like this, but I'm not sure it's
    // even necessary

    /*
    pub fn compose(&mut self, to_compose: &TokenProfits) {
        for profit in &to_compose.profits {
            for existing_profit in self.profits.iter_mut().filter(|p|
                p.profit_collector == profit.profit_collector && p.token_address == profit.token_address
            ) {
                if existing_profit.amount < profit.amount {
                    existing_profit.amount = profit.amount;
                }
            }
        }
    }
     */
}
