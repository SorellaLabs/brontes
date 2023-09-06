use reth_primitives::{Address, U256};

use crate::structured_trace::StructuredTrace;

#[derive(Debug, Clone)]
pub enum Actions {
    Swap(NormalizedSwap),
    Transfer(NormalizedTransfer),
    Mint(NormalizedMint),
    Burn(NormalizedBurn),
    Unclassified(StructuredTrace),
}

#[derive(Debug, Clone)]
pub struct NormalizedSwap {
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

pub trait NormalizedAction: Clone {
    fn get_action(&self) -> &Actions;
}

impl NormalizedAction for Actions {
    fn get_action(&self) -> &Actions {
        &self
    }
}
