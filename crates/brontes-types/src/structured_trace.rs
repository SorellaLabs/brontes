use std::str::FromStr;

use alloy_primitives::{Address, Log, U256};
use alloy_primitives::{Bytes, B256};
use clickhouse::DbRow;
use itertools::Itertools;
use redefined::self_convert_redefined;
use reth_rpc_types::trace::parity::*;
use rkyv::{Archive, Deserialize as rDeserialize, Serialize as rSerialize};
use serde::{ser::SerializeStruct, Deserialize, Serialize};
use serde_with::serde_as;

use crate::{
    constants::{EXECUTE_FFS_YO, SCP_MAIN_CEX_DEX_BOT},
    db::clickhouse_serde::tx_trace::*,
    serde_utils::u256,
};
pub trait TraceActions {
    fn get_callframe_info(&self) -> CallFrameInfo<'_>;
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
        matches!(&self.trace.action, Action::Create(_))
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
        let Some(res) = &self.trace.result else {
            return Bytes::default();
        };
        match res {
            reth_rpc_types::trace::parity::TraceOutput::Call(bytes) => bytes.output.clone(),
            _ => Bytes::default(),
        }
    }

    fn get_callframe_info(&self) -> CallFrameInfo<'_> {
        CallFrameInfo {
            trace_idx: self.trace_idx,
            call_data: self.get_calldata(),
            return_data: self.get_return_calldata(),
            target_address: self.get_to_address(),
            from_address: self.get_from_addr(),
            logs: &self.logs,
            delegate_logs: vec![],
            msg_sender: self.msg_sender,
            msg_value: self.get_msg_value(),
        }
    }
}

#[derive(
    Debug, Clone, Serialize, Deserialize, PartialEq, Eq, rSerialize, rDeserialize, Archive,
)]

pub struct DecodedCallData {
    pub function_name: String,
    pub call_data: Vec<DecodedParams>,
    pub return_data: Vec<DecodedParams>,
}

self_convert_redefined!(DecodedCallData);

#[derive(
    Debug, Clone, Serialize, Deserialize, PartialEq, Eq, rSerialize, rDeserialize, Archive,
)]
pub struct DecodedParams {
    pub field_name: String,
    pub field_type: String,
    pub value: String,
}

self_convert_redefined!(DecodedParams);

#[derive(Debug, Clone)]
pub struct CallFrameInfo<'a> {
    pub trace_idx: u64,
    pub call_data: Bytes,
    pub return_data: Bytes,
    pub target_address: Address,
    pub from_address: Address,
    pub logs: &'a [Log],
    pub delegate_logs: Vec<&'a Log>,
    pub msg_sender: Address,
    pub msg_value: U256,
}

#[derive(Debug, Clone)]
pub struct CallInfo {
    pub trace_idx: u64,
    pub target_address: Address,
    pub from_address: Address,
    pub msg_sender: Address,
    pub msg_value: U256,
}

impl CallFrameInfo<'_> {
    pub fn get_fixed_fields(&self) -> CallInfo {
        CallInfo {
            trace_idx: self.trace_idx,
            target_address: self.target_address,
            from_address: self.from_address,
            msg_sender: self.msg_sender,
            msg_value: self.msg_value,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TransactionTraceWithLogs {
    pub trace: TransactionTrace,
    pub logs: Vec<Log>,
    /// the msg.sender of the trace. This allows us to properly deal with
    /// delegate calls and the headache they cause when it comes to proxies
    pub msg_sender: Address,
    pub trace_idx: u64,
    pub decoded_data: Option<DecodedCallData>,
}

impl TransactionTraceWithLogs {
    pub fn get_msg_value(&self) -> U256 {
        match &self.trace.action {
            Action::Call(c) => c.value,
            Action::Create(c) => c.value,
            Action::Reward(r) => r.value,
            Action::Selfdestruct(_) => U256::ZERO,
        }
    }

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

#[serde_as]
#[derive(Default, Debug, Clone, PartialEq, Deserialize)]
pub struct TxTrace {
    pub block_number: u64,
    pub trace: Vec<TransactionTraceWithLogs>,
    #[serde(with = "u256")]
    pub tx_hash: B256,
    pub gas_used: u128,
    pub effective_price: u128,
    pub tx_index: u64,
    // False if the transaction reverted
    pub is_success: bool,
}

impl TxTrace {
    pub fn new(
        block_number: u64,
        trace: Vec<TransactionTraceWithLogs>,
        tx_hash: B256,
        tx_index: u64,
        gas_used: u128,
        effective_price: u128,
        is_success: bool,
    ) -> Self {
        Self { block_number, trace, tx_hash, tx_index, effective_price, gas_used, is_success }
    }
}

impl Serialize for TxTrace {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut ser_struct = serializer.serialize_struct("TxTrace", 30)?;

        ser_struct.serialize_field("block_number", &self.block_number)?;
        ser_struct.serialize_field("tx_hash", &format!("{:?}", self.tx_hash))?;
        ser_struct.serialize_field("gas_used", &self.gas_used)?;
        ser_struct.serialize_field("effective_price", &self.effective_price)?;
        ser_struct.serialize_field("tx_index", &self.tx_index)?;
        ser_struct.serialize_field("is_success", &self.is_success)?;

        let trace_idx = self.trace.iter().map(|trace| trace.trace_idx).collect_vec();
        ser_struct.serialize_field("trace_meta.trace_idx", &trace_idx)?;

        let msg_sender = self
            .trace
            .iter()
            .map(|trace| format!("{:?}", trace.msg_sender))
            .collect_vec();
        ser_struct.serialize_field("trace_meta.msg_sender", &msg_sender)?;

        let error = self
            .trace
            .iter()
            .map(|trace| trace.trace.error.clone())
            .collect_vec();
        ser_struct.serialize_field("trace_meta.error", &error)?;

        let subtraces = self
            .trace
            .iter()
            .map(|trace| trace.trace.subtraces as u64)
            .collect_vec();
        ser_struct.serialize_field("trace_meta.subtraces", &subtraces)?;

        let trace_address = self
            .trace
            .iter()
            .map(|trace| {
                trace
                    .trace
                    .trace_address
                    .iter()
                    .map(|a| *a as u64)
                    .collect_vec()
            })
            .collect_vec();
        ser_struct.serialize_field("trace_meta.trace_address", &trace_address)?;

        let decoded_data = ClickhouseDecodedCallData::from(self);
        ser_struct.serialize_field("trace_decoded_data.trace_idx", &decoded_data.trace_idx)?;
        ser_struct
            .serialize_field("trace_decoded_data.function_name", &decoded_data.function_name)?;
        ser_struct.serialize_field("trace_decoded_data.call_data", &decoded_data.call_data)?;
        ser_struct.serialize_field("trace_decoded_data.return_data", &decoded_data.return_data)?;

        let logs = ClickhouseLogs::from(self);
        ser_struct.serialize_field("trace_logs.trace_idx", &logs.trace_idx)?;
        ser_struct.serialize_field("trace_logs.log_idx", &logs.log_idx)?;
        ser_struct.serialize_field("trace_logs.address", &logs.address)?;
        ser_struct.serialize_field("trace_logs.topics", &logs.topics)?;
        ser_struct.serialize_field("trace_logs.data", &logs.data)?;

        let create_action = ClickhouseCreateAction::from(self);
        ser_struct.serialize_field("trace_create_actions.trace_idx", &create_action.trace_idx)?;
        ser_struct.serialize_field("trace_create_actions.from", &create_action.from)?;
        ser_struct.serialize_field("trace_create_actions.gas", &create_action.gas)?;
        ser_struct.serialize_field("trace_create_actions.init", &create_action.init)?;
        ser_struct.serialize_field("trace_create_actions.value", &create_action.value)?;

        let call_action = ClickhouseCallAction::from(self);
        ser_struct.serialize_field("trace_call_actions.trace_idx", &call_action.trace_idx)?;
        ser_struct.serialize_field("trace_call_actions.from", &call_action.from)?;
        ser_struct.serialize_field("trace_call_actions.call_type", &call_action.call_type)?;
        ser_struct.serialize_field("trace_call_actions.gas", &call_action.gas)?;
        ser_struct.serialize_field("trace_call_actions.input", &call_action.input)?;
        ser_struct.serialize_field("trace_call_actions.to", &call_action.to)?;
        ser_struct.serialize_field("trace_call_actions.value", &call_action.value)?;

        let self_destruct_action = ClickhouseSelfDestructAction::from(self);
        ser_struct.serialize_field(
            "trace_self_destruct_actions.trace_idx",
            &self_destruct_action.trace_idx,
        )?;
        ser_struct.serialize_field(
            "trace_self_destruct_actions.address",
            &self_destruct_action.address,
        )?;
        ser_struct.serialize_field(
            "trace_self_destruct_actions.balance",
            &self_destruct_action.balance,
        )?;
        ser_struct.serialize_field(
            "trace_self_destruct_actions.refund_address",
            &self_destruct_action.refund_address,
        )?;

        let reward_action = ClickhouseRewardAction::from(self);
        ser_struct.serialize_field("trace_reward_actions.trace_idx", &reward_action.trace_idx)?;
        ser_struct.serialize_field("trace_reward_actions.author", &reward_action.author)?;
        ser_struct.serialize_field("trace_reward_actions.value", &reward_action.value)?;
        ser_struct
            .serialize_field("trace_reward_actions.reward_type", &reward_action.reward_type)?;

        let call_output = ClickhouseCallOutput::from(self);
        ser_struct.serialize_field("trace_call_outputs.trace_idx", &call_output.trace_idx)?;
        ser_struct.serialize_field("trace_call_outputs.gas_used", &call_output.gas_used)?;
        ser_struct.serialize_field("trace_call_outputs.output", &call_output.output)?;

        let create_output = ClickhouseCreateOutput::from(self);
        ser_struct.serialize_field("trace_create_outputs.trace_idx", &create_output.trace_idx)?;
        ser_struct.serialize_field("trace_create_outputs.address", &create_output.address)?;
        ser_struct.serialize_field("trace_create_outputs.code", &create_output.code)?;
        ser_struct.serialize_field("trace_create_outputs.gas_used", &create_output.gas_used)?;

        ser_struct.end()
    }
}

impl DbRow for TxTrace {
    const COLUMN_NAMES: &'static [&'static str] = &[
        "block_number",
        "tx_hash",
        "gas_used",
        "effective_price",
        "tx_index",
        "is_success",
        "trace_meta.trace_idx",
        "trace_meta.msg_sender",
        "trace_meta.error",
        "trace_meta.subtraces",
        "trace_meta.trace_address",
        "trace_decoded_data.trace_idx",
        "trace_decoded_data.function_name",
        "trace_decoded_data.call_data",
        "trace_decoded_data.return_data",
        "trace_logs.trace_idx",
        "trace_logs.log_idx",
        "trace_logs.address",
        "trace_logs.topics",
        "trace_logs.data",
        "trace_create_actions.trace_idx",
        "trace_create_actions.from",
        "trace_create_actions.gas",
        "trace_create_actions.init",
        "trace_create_actions.value",
        "trace_call_actions.trace_idx",
        "trace_call_actions.from",
        "trace_call_actions.call_type",
        "trace_call_actions.gas",
        "trace_call_actions.input",
        "trace_call_actions.to",
        "trace_call_actions.value",
        "trace_self_destruct_actions.trace_idx",
        "trace_self_destruct_actions.address",
        "trace_self_destruct_actions.balance",
        "trace_self_destruct_actions.refund_address",
        "trace_reward_actions.trace_idx",
        "trace_reward_actions.author",
        "trace_reward_actions.reward_type",
        "trace_reward_actions.value",
        "trace_call_outputs.trace_idx",
        "trace_call_outputs.gas_used",
        "trace_call_outputs.output",
        "trace_create_outputs.trace_idx",
        "trace_create_outputs.address",
        "trace_create_outputs.code",
        "trace_create_outputs.gas_used",
    ];
}
