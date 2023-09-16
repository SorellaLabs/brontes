use reth_primitives::{Address, U256};
use reth_rpc_types::{trace::parity::TransactionTrace, Log};

#[derive(Debug, Clone)]
pub enum Actions {
    Swap(NormalizedSwap),
    Transfer(NormalizedTransfer),
    Mint(NormalizedMint),
    Burn(NormalizedBurn),
    Unclassified(TransactionTrace, Vec<Log>)
}

impl Actions {
    pub fn get_logs(&self) -> Vec<Log> {
        match self {
            Self::Unclassified(_, log) => log.clone(),
            _ => vec![]
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

    pub fn is_unclassified(&self) -> bool {
        matches!(self, Actions::Unclassified(_, _))
    }
}

//TODO(Will) : call address is a bit weird of a name here, we should just call
// it swapper TODO (Will): but i guess caller might be more precise if you are
// considering that a swap that transfers TODO(Will): to a diff address is a
// swap + transfer
#[derive(Debug, Clone)]
pub struct NormalizedSwap {
    pub index:        u64,
    pub call_address: Address,
    pub token_in:     Address,
    pub token_out:    Address,
    pub amount_in:    U256,
    pub amount_out:   U256
}

#[derive(Debug, Clone)]
pub struct NormalizedTransfer {
    pub index:  u64,
    pub to:     Address,
    pub from:   Address,
    pub token:  Address,
    pub amount: U256
}

#[derive(Debug, Clone)]
pub struct NormalizedMint {
    pub index:  u64,
    pub to:     Address,
    pub token:  Vec<Address>,
    pub amount: Vec<U256>
}

#[derive(Debug, Clone)]
pub struct NormalizedBurn {
    pub index:  u64,
    pub from:   Address,
    pub token:  Vec<Address>,
    pub amount: Vec<U256>
}

pub struct NormalizedLiquidation {
    pub index:      u64,
    pub liquidator: Address,
    pub liquidatee: Address,
    pub token:      Address,
    pub amount:     U256,
    pub reward:     U256
}

pub trait NormalizedAction: Send + Sync + Clone {
    fn get_action(&self) -> &Actions;
}

impl NormalizedAction for Actions {
    fn get_action(&self) -> &Actions {
        self
    }
}
