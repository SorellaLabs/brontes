use reth_primitives::{Address, Log, U256};

use crate::structured_trace::StructuredTrace;

#[derive(Debug, Clone, Default)]
pub enum Actions {
    Swap(NormalizedSwap),
    Transfer(NormalizedTransfer),
    Mint(NormalizedMint),
    Burn(NormalizedBurn),
    Unclassified(StructuredTrace, Vec<Log>),
    #[default]
    None,
}

#[derive(Debug, Clone)]
pub struct NormalizedSwap {
    pub address: Address,
    pub token_in: Address,
    pub token_out: Address,
    pub amount_in: U256,
    pub amount_out: U256,
}

#[derive(Debug, Clone)]
pub struct NormalizedTransfer {
    pub from: Address,
    pub to: Address,
    pub token: Address,
    pub amount: U256,
}

#[derive(Debug, Clone)]
pub struct NormalizedMint {
    pub from: Address,
    pub token: Address,
    pub amount: U256,
}

#[derive(Debug, Clone)]
pub struct NormalizedBurn {
    pub from: Address,
    pub token: Address,
    pub amount: U256,
}

pub trait NormalizedAction: Send + Clone {
    fn get_action(&self) -> &Actions;
}

impl NormalizedAction for Actions {
    fn get_action(&self) -> &Actions {
        &self
    }
}
