use alloy_dyn_abi::DynSolValue;
use reth_primitives::{Address, Bytes, Log, H256, U256};
use reth_rpc_types::trace::parity::{
    Action, CreateAction, RewardAction, SelfdestructAction, TransactionTrace,
};

use crate::{
    normalized_actions::Actions,
    tree::{Node, Root},
};

pub trait TraceActions {
    fn get_from_addr(&self) -> Address;
    fn get_calldata(&self) -> Bytes;
}

impl TraceActions for TransactionTrace {
    fn get_from_addr(&self) -> Address {
        match &self.action {
            Action::Call(call) => call.from,
            Action::Create(call) => call.from,
            Action::Reward(call) => call.author,
            Action::Selfdestruct(call) => call.address,
        }
    }
    fn get_calldata(&self) -> Bytes {
        match &self.action {
            Action::Call(call) => call.input.clone(),
            Action::Create(call) => call.init.clone(),
            _ => Bytes::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TxTrace {
    pub trace: Vec<TransactionTrace>,
    pub logs: Vec<Log>,
    pub tx_hash: H256,
    pub tx_index: usize,
}

impl TxTrace {
    pub fn new(
        trace: Vec<TransactionTrace>,
        tx_hash: H256,
        logs: Vec<Log>,
        tx_index: usize,
    ) -> Self {
        Self { trace, tx_hash, tx_index, logs }
    }
}
