use reth_primitives::{Address, Bytes, H160, H256};
use reth_rpc_types::{
    trace::parity::{Action, TransactionTrace},
    Log,
};

pub trait TraceActions {
    fn get_from_addr(&self) -> Address;
    fn get_to_address(&self) -> Address;
    fn get_calldata(&self) -> Bytes;
    fn get_return_calldata(&self) -> Bytes;
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

    fn get_to_address(&self) -> Address {
        match &self.action {
            Action::Call(call) => call.to,
            Action::Create(call) => H160::default(),
            Action::Reward(call) => H160::default(),
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

    fn get_return_calldata(&self) -> Bytes {
        let Some(res) = &self.result else { return Bytes::default() };
        match res {
            reth_rpc_types::trace::parity::TraceOutput::Call(bytes) => bytes.output.clone(),
            _ => Bytes::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TxTrace {
    pub trace:           Vec<TransactionTrace>,
    pub logs:            Vec<Log>,
    pub tx_hash:         H256,
    pub gas_used:        u64,
    pub effective_price: u64,
    pub tx_index:        u64,
}

impl TxTrace {
    pub fn new(
        trace: Vec<TransactionTrace>,
        tx_hash: H256,
        logs: Vec<Log>,
        tx_index: u64,
        gas_used: u64,
        effective_price: u64,
    ) -> Self {
        Self { trace, tx_hash, tx_index, logs, effective_price, gas_used }
    }
}
