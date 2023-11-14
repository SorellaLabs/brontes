use alloy_dyn_abi::DynSolValue;
use reth_primitives::{Address, Bytes, H160, H256};
use reth_rpc_types::{
    trace::parity::{Action, TransactionTrace},
    Log,
};
use serde::{Deserialize, Serialize};

pub trait TraceActions {
    fn get_from_addr(&self) -> Address;
    fn get_to_address(&self) -> Address;
    fn get_calldata(&self) -> Bytes;
    fn get_return_calldata(&self) -> Bytes;
}

impl TraceActions for TransactionTraceWithLogs {
    fn get_from_addr(&self) -> Address {
        match &self.trace.action {
            Action::Call(call) => call.from,
            Action::Create(call) => call.from,
            Action::Reward(call) => call.author,
            Action::Selfdestruct(call) => call.address,
        }
    }

    fn get_to_address(&self) -> Address {
        match &self.trace.action {
            Action::Call(call) => call.to,
            Action::Create(_) => H160::default(),
            Action::Reward(_) => H160::default(),
            Action::Selfdestruct(call) => call.address,
        }
    }

    fn get_calldata(&self) -> Bytes {
        match &self.trace.action {
            Action::Call(call) => call.input.clone(),
            Action::Create(call) => call.init.clone(),
            _ => Bytes::default(),
        }
    }

    fn get_return_calldata(&self) -> Bytes {
        let Some(res) = &self.trace.result else { return Bytes::default() };
        match res {
            reth_rpc_types::trace::parity::TraceOutput::Call(bytes) => bytes.output.clone(),
            _ => Bytes::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecodedCallData {
    pub function_name: String,
    pub call_data:     Vec<DecodedParams>,
    pub return_data:   Vec<DecodedParams>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecodedParams {
    pub field_name: String,
    pub field_type: String,
    pub value:      String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionTraceWithLogs {
    pub trace:        TransactionTrace,
    pub decoded_data: Option<DecodedCallData>,
    pub logs:         Vec<Log>,
    pub trace_idx:    u64,
}

#[derive(Debug, Clone)]
pub struct TxTrace {
    pub trace:           Vec<TransactionTraceWithLogs>,
    pub tx_hash:         H256,
    pub gas_used:        u64,
    pub effective_price: u64,
    pub tx_index:        u64,
}

impl TxTrace {
    pub fn new(
        trace: Vec<TransactionTraceWithLogs>,
        tx_hash: H256,
        tx_index: u64,
        gas_used: u64,
        effective_price: u64,
    ) -> Self {
        Self { trace, tx_hash, tx_index, effective_price, gas_used }
    }
}
