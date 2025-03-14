use alloy_primitives::{Log, LogData};
use alloy_rpc_types_trace::parity::{
    Action, CallAction, CallOutput, CallType, CreateAction, CreateOutput, CreationMethod,
    RewardAction, RewardType, SelfdestructAction, TraceOutput, TransactionTrace,
};
use clickhouse::Row;
use redefined::Redefined;
use rkyv::{Archive, Deserialize as rDeserialize, Serialize as rSerialize};
use serde::{Deserialize, Serialize};

use super::redefined_types::primitives::*;
use crate::{
    implement_table_value_codecs_with_zc,
    structured_trace::{DecodedCallData, TransactionTraceWithLogs, TxTrace},
};

#[derive(Debug, Default, PartialEq, Row, Clone, Serialize, Deserialize, Redefined)]
#[redefined_attr(derive(Debug, PartialEq, Clone, Serialize, rSerialize, rDeserialize, Archive))]
pub struct TxTracesInner {
    pub traces: Option<Vec<TxTrace>>,
}

impl TxTracesInner {
    pub fn new(traces: Option<Vec<TxTrace>>) -> Self {
        Self { traces }
    }
}

implement_table_value_codecs_with_zc!(TxTracesInnerRedefined);

#[derive(
    Debug,
    Clone,
    Redefined,
    PartialEq,
    serde::Serialize,
    rSerialize,
    rDeserialize,
    rkyv::Archive,
    Default,
)]
#[redefined(TxTrace)]
pub struct TxTraceRedefined {
    pub block_number: u64,
    pub trace: Vec<TransactionTraceWithLogsRedefined>,
    pub tx_hash: FixedBytesRedefined<32>,
    pub gas_used: u128,
    pub effective_price: u128,
    pub tx_index: u64,
    // False if the transaction reverted
    pub is_success: bool,
}

#[derive(
    Debug, Clone, Redefined, PartialEq, serde::Serialize, rSerialize, rDeserialize, rkyv::Archive,
)]
#[redefined(TransactionTraceWithLogs)]
pub struct TransactionTraceWithLogsRedefined {
    pub trace: TransactionTraceRedefined,
    pub logs: Vec<LogRedefined>,
    pub msg_sender: AddressRedefined,
    pub trace_idx: u64,
    pub decoded_data: Option<DecodedCallData>,
}

#[derive(
    Debug, Clone, Redefined, PartialEq, serde::Serialize, rSerialize, rDeserialize, rkyv::Archive,
)]
#[redefined(Log)]
pub struct LogRedefined {
    pub address: AddressRedefined,
    pub data: LogDataRedefined,
}

#[derive(
    Debug, Clone, Redefined, PartialEq, serde::Serialize, rSerialize, rDeserialize, rkyv::Archive,
)]
#[redefined(LogData)]
#[redefined_attr(to_source = "LogData::new_unchecked(self.topics.iter().copied().map(Into::into).\
                              collect(), self.data.into())")]
pub struct LogDataRedefined {
    #[redefined(func = "src.topics().to_vec()")]
    pub topics: Vec<FixedBytesRedefined<32>>,
    pub data: BytesRedefined,
}

#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    serde::Serialize,
    rSerialize,
    rDeserialize,
    rkyv::Archive,
    Redefined,
)]
#[redefined(TransactionTrace)]
pub struct TransactionTraceRedefined {
    pub action: ActionRedefined,
    pub error: Option<String>,
    pub result: Option<TraceOutputRedefined>,
    pub subtraces: usize,
    pub trace_address: Vec<usize>,
}

#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    serde::Serialize,
    rSerialize,
    rDeserialize,
    rkyv::Archive,
    Redefined,
)]
#[redefined(Action)]
pub enum ActionRedefined {
    Call(CallActionRedefined),
    Create(CreateActionRedefined),
    Selfdestruct(SelfdestructActionRedefined),
    Reward(RewardActionRedefined),
}

#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    serde::Serialize,
    rSerialize,
    rDeserialize,
    rkyv::Archive,
    Redefined,
)]
#[redefined(CallAction)]
pub struct CallActionRedefined {
    pub from: AddressRedefined,
    pub call_type: CallTypeRedefined,
    pub gas: u64,
    pub input: BytesRedefined,
    pub to: AddressRedefined,
    pub value: U256Redefined,
}

#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    serde::Serialize,
    rSerialize,
    rDeserialize,
    rkyv::Archive,
    Redefined,
)]
#[redefined(CreateAction)]
pub struct CreateActionRedefined {
    pub from: AddressRedefined,
    pub gas: u64,
    pub init: BytesRedefined,
    pub value: U256Redefined,
    pub creation_method: CreationMethodRedefined,
}

#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    serde::Serialize,
    rSerialize,
    rDeserialize,
    rkyv::Archive,
    Redefined,
)]
#[redefined(CreationMethod)]
pub enum CreationMethodRedefined {
    None,
    Create,
    Create2,
    EofCreate,
}

#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    serde::Serialize,
    rSerialize,
    rDeserialize,
    rkyv::Archive,
    Redefined,
)]
#[redefined(SelfdestructAction)]
pub struct SelfdestructActionRedefined {
    pub address: AddressRedefined,
    pub balance: U256Redefined,
    pub refund_address: AddressRedefined,
}

#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    serde::Serialize,
    rSerialize,
    rDeserialize,
    rkyv::Archive,
    Redefined,
)]
#[redefined(RewardAction)]
pub struct RewardActionRedefined {
    pub author: AddressRedefined,
    pub reward_type: RewardTypeRedefined,
    pub value: U256Redefined,
}

#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    serde::Serialize,
    rSerialize,
    rDeserialize,
    rkyv::Archive,
    Redefined,
)]
#[redefined(RewardType)]
pub enum RewardTypeRedefined {
    Block,
    Uncle,
}

#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    serde::Serialize,
    rSerialize,
    rDeserialize,
    rkyv::Archive,
    Redefined,
)]
#[redefined(CallType)]
pub enum CallTypeRedefined {
    None,
    Call,
    CallCode,
    DelegateCall,
    StaticCall,
    AuthCall,
}

#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    serde::Serialize,
    rSerialize,
    rDeserialize,
    rkyv::Archive,
    Redefined,
)]
#[redefined(TraceOutput)]
pub enum TraceOutputRedefined {
    Call(CallOutputRedefined),
    Create(CreateOutputRedefined),
}

#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    serde::Serialize,
    rSerialize,
    rDeserialize,
    rkyv::Archive,
    Redefined,
)]
#[redefined(CallOutput)]
pub struct CallOutputRedefined {
    pub gas_used: u64,
    pub output: BytesRedefined,
}

#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    serde::Serialize,
    rSerialize,
    rDeserialize,
    rkyv::Archive,
    Redefined,
)]
#[redefined(CreateOutput)]
pub struct CreateOutputRedefined {
    pub address: AddressRedefined,
    pub code: BytesRedefined,
    pub gas_used: u64,
}
