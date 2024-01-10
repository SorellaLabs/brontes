use alloy_primitives::LogData;
use reth_primitives::{Address, Bytes, B256};
use reth_rpc_types::trace::parity::{Action, CallType, StateDiff, TransactionTrace};
use serde::{Deserialize, Serialize};

pub trait TraceActions {
    fn get_from_addr(&self) -> Address;
    fn get_to_address(&self) -> Address;
    fn get_calldata(&self) -> Bytes;
    fn get_return_calldata(&self) -> Bytes;
    fn is_static_call(&self) -> bool;
}

impl TraceActions for TransactionTraceWithLogs {
    fn is_static_call(&self) -> bool {
        match &self.trace.action {
            Action::Call(call) => call.call_type == CallType::StaticCall,
            _ => false,
        }
    }

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
            Action::Create(_) => Address::default(),
            Action::Reward(_) => Address::default(),
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
    pub logs:         Vec<LogData>,
    pub trace_idx:    u64,
}

impl TransactionTraceWithLogs {
    pub fn get_trace_address(&self) -> Vec<usize> {
        self.trace.trace_address.clone()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxTrace {
    pub trace:           Vec<TransactionTraceWithLogs>,
    pub state_diff:      StateDiff,
    pub tx_hash:         B256,
    pub gas_used:        u128,
    pub effective_price: u128,
    pub tx_index:        u64,
    pub is_success:      bool,
}

impl TxTrace {
    pub fn new(
        trace: Vec<TransactionTraceWithLogs>,
        tx_hash: B256,
        tx_index: u64,
        gas_used: u128,
        effective_price: u128,
        is_success: bool,
        state_diff: StateDiff,
    ) -> Self {
        Self { trace, tx_hash, tx_index, effective_price, gas_used, is_success, state_diff }
    }
}
