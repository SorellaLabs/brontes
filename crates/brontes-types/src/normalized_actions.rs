use std::{collections::HashMap, fmt::Debug};

use reth_primitives::{Address, U256};
use reth_rpc_types::Log;
use serde::{Deserialize, Serialize};
use sorella_db_databases::clickhouse::{self, InsertRow, Row};

use crate::structured_trace::TransactionTraceWithLogs;

#[derive(Debug, Clone, Deserialize)]
pub enum Actions {
    Swap(NormalizedSwap),
    Transfer(NormalizedTransfer),
    Mint(NormalizedMint),
    Burn(NormalizedBurn),
    Collect(NormalizedCollect),
    Unclassified(TransactionTraceWithLogs),
}

impl InsertRow for Actions {
    fn get_column_names(&self) -> &'static [&'static str] {
        match self {
            Actions::Swap(_) => NormalizedSwap::COLUMN_NAMES,
            Actions::Transfer(_) => NormalizedTransfer::COLUMN_NAMES,
            Actions::Mint(_) => NormalizedMint::COLUMN_NAMES,
            Actions::Burn(_) => NormalizedBurn::COLUMN_NAMES,
            Actions::Collect(_) => NormalizedCollect::COLUMN_NAMES,
            Actions::Unclassified(..) => panic!(),
        }
    }
}

impl Serialize for Actions {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Actions::Swap(s) => s.serialize(serializer),
            Actions::Mint(m) => m.serialize(serializer),
            Actions::Transfer(t) => t.serialize(serializer),
            Actions::Burn(b) => b.serialize(serializer),
            Actions::Collect(c) => c.serialize(serializer),
            Actions::Unclassified(trace) => (trace).serialize(serializer),
        }
    }
}

impl Actions {
    pub fn force_swap(self) -> NormalizedSwap {
        match self {
            Actions::Swap(s) => s,
            _ => unreachable!(),
        }
    }

    pub fn get_logs(&self) -> Vec<Log> {
        match self {
            Self::Unclassified(a) => a.logs.clone(),
            _ => vec![],
        }
    }

    pub fn get_to_address(&self) -> Address {
        match self {
            Actions::Swap(s) => s.pool,
            Actions::Mint(m) => m.to,
            Actions::Burn(b) => b.to,
            Actions::Transfer(t) => t.to,
            Actions::Collect(c) => c.to,
            Actions::Unclassified(t) => match &t.trace.action {
                reth_rpc_types::trace::parity::Action::Call(c) => c.to,
                reth_rpc_types::trace::parity::Action::Create(_) => Address::zero(),
                reth_rpc_types::trace::parity::Action::Reward(_) => Address::zero(),
                reth_rpc_types::trace::parity::Action::Selfdestruct(s) => s.address,
            },
        }
    }

    pub fn is_swap(&self) -> bool {
        matches!(self, Actions::Swap(_))
    }

    pub fn is_burn(&self) -> bool {
        matches!(self, Actions::Burn(_))
    }

    pub fn is_mint(&self) -> bool {
        matches!(self, Actions::Mint(_))
    }

    pub fn is_transfer(&self) -> bool {
        matches!(self, Actions::Transfer(_))
    }

    pub fn is_collect(&self) -> bool {
        matches!(self, Actions::Collect(_))
    }

    pub fn is_unclassified(&self) -> bool {
        matches!(self, Actions::Unclassified(_))
    }
}

#[derive(Debug, Serialize, Clone, Row, PartialEq, Eq, Deserialize)]
pub struct NormalizedFlashLoan {
    pub index:    u64,
    pub from:     Address,
    pub pool:     Address,
    pub token:    Address,
    pub fee_paid: Address,
}

#[derive(Debug, Default, Serialize, Clone, Row, PartialEq, Eq, Deserialize)]
pub struct NormalizedSwap {
    pub index:      u64,
    pub from:       Address,
    pub pool:       Address,
    pub token_in:   Address,
    pub token_out:  Address,
    pub amount_in:  U256,
    pub amount_out: U256,
}

#[derive(Debug, Serialize, Clone, Row, PartialEq, Eq, Deserialize)]
pub struct NormalizedTransfer {
    pub index:  u64,
    pub to:     Address,
    pub from:   Address,
    pub token:  Address,
    pub amount: U256,
}

#[derive(Debug, Serialize, Clone, Row, PartialEq, Eq, Deserialize)]
pub struct NormalizedMint {
    pub index:     u64,
    pub from:      Address,
    pub to:        Address,
    pub recipient: Address,
    pub token:     Vec<Address>,
    pub amount:    Vec<U256>,
}

#[derive(Debug, Serialize, Clone, Row, PartialEq, Eq, Deserialize)]
pub struct NormalizedBurn {
    pub index:     u64,
    pub from:      Address,
    pub to:        Address,
    pub recipient: Address,
    pub token:     Vec<Address>,
    pub amount:    Vec<U256>,
}

#[derive(Debug, Serialize, Clone, Row, PartialEq, Eq, Deserialize)]
pub struct NormalizedCollect {
    pub index:     u64,
    pub to:        Address,
    pub from:      Address,
    pub recipient: Address,
    pub token:     Vec<Address>,
    pub amount:    Vec<U256>,
}

#[derive(Debug, Serialize, Clone, Row, PartialEq, Eq, Deserialize)]
pub struct NormalizedLiquidation {
    pub index:      u64,
    pub liquidator: Address,
    pub liquidatee: Address,
    pub token:      Address,
    pub amount:     U256,
    pub reward:     U256,
}

#[derive(Debug, Serialize, Clone, Row, PartialEq, Eq, Deserialize)]
pub struct NormalizedLoan {
    pub index:        u64,
    pub lender:       Address,
    pub borrower:     Address,
    pub loaned_token: Address,
    pub loan_amount:  U256,
    pub collateral:   HashMap<Address, U256>,
}

#[derive(Debug, Serialize, Clone, Row, PartialEq, Eq, Deserialize)]
pub struct NormalizedRepayment {
    pub index:            u64,
    pub lender:           Address,
    pub borrower:         Address,
    pub repayed_token:    Address,
    pub repayment_amount: U256,
    pub collateral:       HashMap<Address, U256>,
}

pub trait NormalizedAction: Debug + Send + Sync + Clone {
    fn get_action(&self) -> &Actions;
}

impl NormalizedAction for Actions {
    fn get_action(&self) -> &Actions {
        self
    }
}
