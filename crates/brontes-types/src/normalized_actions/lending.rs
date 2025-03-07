use std::fmt::Debug;

use alloy_primitives::{Address, U256};
use clickhouse::Row;
use malachite::Rational;
use serde::{Deserialize, Serialize};

use crate::{db::token_info::TokenInfoWithAddress, FastHashMap, Protocol};

#[derive(Debug, Serialize, Clone, Row, PartialEq, Eq, Deserialize)]
pub struct NormalizedLoan {
    pub protocol:     Protocol,
    pub trace_index:  u64,
    pub lender:       Address,
    pub borrower:     Address,
    pub loaned_token: TokenInfoWithAddress,
    pub loan_amount:  Rational,
    pub collateral:   FastHashMap<TokenInfoWithAddress, Rational>,
    pub msg_value:    U256,
}

#[derive(Debug, Serialize, Clone, Row, PartialEq, Eq, Deserialize)]
pub struct NormalizedRepayment {
    pub protocol:         Protocol,
    pub trace_index:      u64,
    pub lender:           Address,
    pub borrower:         Address,
    pub repayed_token:    TokenInfoWithAddress,
    pub repayment_amount: Rational,
    pub collateral:       FastHashMap<TokenInfoWithAddress, Rational>,
    pub msg_value:        U256,
}
