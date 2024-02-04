use std::str::FromStr;

use alloy_primitives::{Address, Log};
use alloy_sol_types::private::LogData;
use redefined::{redefined_remote, self_convert_redefined, Redefined};
use reth_primitives::{Bytes, B256};
use reth_rpc_types::trace::parity::*;
use rkyv::{Archive, Deserialize as rDeserialize, Serialize as rSerialize};
use serde::{Deserialize, Serialize};

use crate::{
    constants::{EXECUTE_FFS_YO, SCP_MAIN_CEX_DEX_BOT},
    db::redefined_types::primitives::*,
};
pub trait TraceActions {
    fn get_from_addr(&self) -> Address;
    fn get_to_address(&self) -> Address;
    fn get_msg_sender(&self) -> Address;
    fn get_calldata(&self) -> Bytes;
    fn get_return_calldata(&self) -> Bytes;
    fn is_static_call(&self) -> bool;
    fn is_create(&self) -> bool;
    fn action_type(&self) -> &Action;
    fn get_create_output(&self) -> Address;
    fn is_delegate_call(&self) -> bool;
}

impl TraceActions for TransactionTraceWithLogs {
    fn is_static_call(&self) -> bool {
        match &self.trace.action {
            Action::Call(call) => call.call_type == CallType::StaticCall,
            _ => false,
        }
    }

    fn is_create(&self) -> bool {
        match &self.trace.action {
            Action::Create(_) => true,
            _ => false,
        }
    }

    fn is_delegate_call(&self) -> bool {
        match &self.trace.action {
            Action::Call(c) => c.call_type == CallType::DelegateCall,
            _ => false,
        }
    }

    fn get_create_output(&self) -> Address {
        match &self.trace.result {
            Some(TraceOutput::Create(o)) => o.address,
            _ => Address::default(),
        }
    }

    fn action_type(&self) -> &Action {
        &self.trace.action
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

#[derive(
    Debug, Clone, Serialize, Deserialize, PartialEq, Eq, rSerialize, rDeserialize, Archive,
)]

pub struct DecodedCallData {
    pub function_name: String,
    pub call_data:     Vec<DecodedParams>,
    pub return_data:   Vec<DecodedParams>,
}

self_convert_redefined!(DecodedCallData);

#[derive(
    Debug, Clone, Serialize, Deserialize, PartialEq, Eq, rSerialize, rDeserialize, Archive,
)]
pub struct DecodedParams {
    pub field_name: String,
    pub field_type: String,
    pub value:      String,
}

self_convert_redefined!(DecodedParams);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Redefined)]
#[redefined_attr(derive(Debug, PartialEq, Clone, Serialize, rSerialize, rDeserialize, Archive))]
pub struct TransactionTraceWithLogs {
    pub trace:        TransactionTrace,
    #[redefined(func = "self.logs")]
    pub logs:         Vec<Log>,
    /// the msg.sender of the trace. This allows us to properly deal with
    /// delegate calls and the headache they cause when it comes to proxies
    pub msg_sender:   Address,
    pub trace_idx:    u64,
    #[redefined(same_fields)]
    pub decoded_data: Option<DecodedCallData>,
}

impl TransactionTraceWithLogs {
    pub fn get_trace_address(&self) -> Vec<usize> {
        self.trace.trace_address.clone()
    }

    /// Returns true if the call is a call to SCP's mev bot or their notorious
    /// `executeFFsYo` function
    // TODO: Find a better way to track certain contracts / calls that we 100% know
    // are cex-dex
    pub fn is_cex_dex_call(&self) -> bool {
        match &self.trace.action {
            Action::Call(call) => {
                // Assuming SCP_MAIN_CEX_DEX_BOT is of type Address and is correctly imported
                call.to == SCP_MAIN_CEX_DEX_BOT
                    || call.to
                        == Address::from_str("0xfbEedCFe378866DaB6abbaFd8B2986F5C1768737").unwrap()
                    || (call.input.len() >= 4 && &call.input[0..4] == EXECUTE_FFS_YO.as_ref())
            }
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Redefined)]
#[redefined_attr(derive(Debug, PartialEq, Clone, Serialize, rSerialize, rDeserialize, Archive))]
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

redefined_remote!(
    #[derive(Debug, PartialEq, Clone, Serialize, rSerialize, rDeserialize, Archive)]
    [
        TransactionTrace, TraceOutput, Action,
        CallOutput, CallAction, CreateOutput,
        CreateAction, SelfdestructAction, RewardAction,
        RewardType
    ] : "alloy-rpc-types"
);

redefined_remote!(
    #[derive(Default, Debug, PartialEq, Clone, Serialize, rSerialize, rDeserialize, Archive)]
    [CallType] : "alloy-rpc-types"
);

#[derive(Debug, PartialEq, Clone, Serialize, rSerialize, rDeserialize, Archive, Redefined)]
#[redefined(Log)]
#[redefined_attr(
    to_source = "Log {address: self.address.into(), data: self.data.into()}",
    from_source = "LogRedefined {address: src.address.into(), data: src.data.into()}"
)]
pub struct LogRedefined {
    pub address: AddressRedefined,
    pub data:    LogDataRedefined,
}

#[derive(Debug, PartialEq, Clone, Serialize, rSerialize, rDeserialize, Archive, Redefined)]
#[redefined(LogData)]
#[redefined_attr(
    to_source = "LogData::new_unchecked(self.topics.to_source(), self.data.to_source())"
)]
pub struct LogDataRedefined {
    #[redefined(func = "src.topics().to_vec().into()")]
    topics:   Vec<B256Redefined>,
    pub data: BytesRedefined,
}
