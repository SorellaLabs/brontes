use alloy_primitives::Log;
use alloy_rlp::{
    BufMut, Decodable, Encodable, RlpDecodable, RlpDecodableWrapper, RlpEncodable,
    RlpEncodableWrapper,
};
use reth_primitives::{Address, Bytes, B256};
use reth_rpc_types::trace::parity::{Action, CallType, TransactionTrace};
use serde::{Deserialize, Serialize};
pub trait TraceActions {
    fn get_from_addr(&self) -> Address;
    fn get_to_address(&self) -> Address;
    fn get_msg_sender(&self) -> Address;
    fn get_calldata(&self) -> Bytes;
    fn get_return_calldata(&self) -> Bytes;
    fn is_static_call(&self) -> bool;
    fn is_delegate_call(&self) -> bool;
}

impl TraceActions for TransactionTraceWithLogs {
    fn is_static_call(&self) -> bool {
        match &self.trace.action {
            Action::Call(call) => call.call_type == CallType::StaticCall,
            _ => false,
        }
    }

    fn is_delegate_call(&self) -> bool {
        match &self.trace.action {
            Action::Call(c) => c.call_type == CallType::DelegateCall,
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

    fn get_msg_sender(&self) -> Address {
        self.msg_sender
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DecodedCallData {
    pub function_name: String,
    pub call_data:     Vec<DecodedParams>,
    pub return_data:   Vec<DecodedParams>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DecodedParams {
    pub field_name: String,
    pub field_type: String,
    pub value:      String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TransactionTraceWithLogs {
    pub trace:        TransactionTrace,
    pub logs:         Vec<Log>,
    /// the msg.sender of the trace. This allows us to properly deal with
    /// delegate calls and the headache they cause when it comes to proxies
    pub msg_sender:   Address,
    pub trace_idx:    u64,
    pub decoded_data: Option<DecodedCallData>,
}

impl TransactionTraceWithLogs {
    pub fn get_trace_address(&self) -> Vec<usize> {
        self.trace.trace_address.clone()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxTrace {
    pub trace:           Vec<TransactionTraceWithLogs>,
    pub tx_hash:         B256,
    pub gas_used:        u128,
    pub effective_price: u128,
    pub tx_index:        u64,
    // False if the transaction reverted
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
    ) -> Self {
        Self { trace, tx_hash, tx_index, effective_price, gas_used, is_success }
    }
}
