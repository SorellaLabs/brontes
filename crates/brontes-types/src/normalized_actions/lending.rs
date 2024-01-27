use std::{
    collections::HashMap,
    fmt::{Debug},
};



use reth_primitives::{Address, U256};

use serde::{Deserialize, Serialize};
use sorella_db_databases::{
    clickhouse,
    clickhouse::{Row},
};

#[derive(Debug, Serialize, Clone, Row, PartialEq, Eq, Deserialize)]
pub struct NormalizedLoan {
    pub trace_index:  u64,
    pub lender:       Address,
    pub borrower:     Address,
    pub loaned_token: Address,
    pub loan_amount:  U256,
    pub collateral:   HashMap<Address, U256>,
}

#[derive(Debug, Serialize, Clone, Row, PartialEq, Eq, Deserialize)]
pub struct NormalizedRepayment {
    pub trace_index:      u64,
    pub lender:           Address,
    pub borrower:         Address,
    pub repayed_token:    Address,
    pub repayment_amount: U256,
    pub collateral:       HashMap<Address, U256>,
}
