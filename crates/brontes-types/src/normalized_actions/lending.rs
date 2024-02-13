use std::{collections::HashMap, fmt::Debug};

use alloy_primitives::U256;
use malachite::Rational;
use reth_primitives::Address;
use serde::{Deserialize, Serialize};
use sorella_db_databases::{clickhouse, clickhouse::Row};

use crate::{db::token_info::TokenInfoWithAddress, Protocol};

#[derive(Debug, Serialize, Clone, Row, PartialEq, Eq, Deserialize)]
pub struct NormalizedLoan {
    pub protocol: Protocol,
    pub trace_index: u64,
    pub lender: Address,
    pub borrower: Address,
    pub loaned_token: TokenInfoWithAddress,
    pub loan_amount: Rational,
    pub collateral: HashMap<TokenInfoWithAddress, Rational>,
    pub msg_value: U256,
}

#[derive(Debug, Serialize, Clone, Row, PartialEq, Eq, Deserialize)]
pub struct NormalizedRepayment {
    pub protocol: Protocol,
    pub trace_index: u64,
    pub lender: Address,
    pub borrower: Address,
    pub repayed_token: TokenInfoWithAddress,
    pub repayment_amount: Rational,
    pub collateral: HashMap<TokenInfoWithAddress, Rational>,
    pub msg_value: U256,
}
