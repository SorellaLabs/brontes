use alloy_primitives::{Log, LogData};
use brontes_types::{
    db::{
        redefined_types::primitives::{
            Redefined_Address, Redefined_Alloy_Bytes, Redefined_FixedBytes, Redefined_U256,
            Redefined_U64,
        },
        traces::TxTracesInner,
    },
    structured_trace::{DecodedCallData, TransactionTraceWithLogs, TxTrace},
};
use redefined::{Redefined, RedefinedConvert};
use reth_rpc_types::trace::parity::{
    Action, CallAction, CallOutput, CallType, CreateAction, CreateOutput, RewardAction, RewardType,
    SelfdestructAction, TraceOutput, TransactionTrace,
};
use serde_with::serde_as;
use sorella_db_databases::{clickhouse, clickhouse::Row};

use super::{LibmdbxData, ReturnKV};
use crate::libmdbx::TxTraces;

#[serde_as]
#[derive(Debug, Clone, Row, serde::Serialize, serde::Deserialize)]
pub struct TxTracesData {
    pub block_number: u64,
    pub inner:        TxTracesInner,
}

impl LibmdbxData<TxTraces> for TxTracesData {
    fn into_key_val(&self) -> ReturnKV<TxTraces> {
        (self.block_number, self.inner.clone()).into()
    }
}

#[serde_as]
#[derive(
    Debug,
    Default,
    Clone,
    Redefined,
    serde::Serialize,
    serde::Deserialize,
    rkyv::Serialize,
    rkyv::Deserialize,
    rkyv::Archive,
)]
#[redefined(TxTracesInner)]
pub struct LibmdbxTxTracesInner {
    pub traces: Option<Vec<LibmdbxTxTrace>>,
}

#[derive(
    Debug,
    Clone,
    Redefined,
    serde::Serialize,
    serde::Deserialize,
    rkyv::Serialize,
    rkyv::Deserialize,
    rkyv::Archive,
)]
#[redefined(TxTrace)]
pub struct LibmdbxTxTrace {
    pub trace:           Vec<LibmdbxTransactionTraceWithLogs>,
    pub tx_hash:         Redefined_FixedBytes<32>,
    pub gas_used:        u128,
    pub effective_price: u128,
    pub tx_index:        u64,
    // False if the transaction reverted
    pub is_success:      bool,
}

#[derive(
    Debug,
    Clone,
    Redefined,
    serde::Serialize,
    serde::Deserialize,
    rkyv::Serialize,
    rkyv::Deserialize,
    rkyv::Archive,
)]
#[redefined(TransactionTraceWithLogs)]
pub struct LibmdbxTransactionTraceWithLogs {
    pub trace:        LibmdbxTransactionTrace,
    pub logs:         Vec<LibmdbxLog>,
    pub msg_sender:   Redefined_Address,
    pub trace_idx:    u64,
    pub decoded_data: Option<DecodedCallData>,
}

#[derive(
    Debug,
    Clone,
    Redefined,
    serde::Serialize,
    serde::Deserialize,
    rkyv::Serialize,
    rkyv::Deserialize,
    rkyv::Archive,
)]
#[redefined(Log)]
pub struct LibmdbxLog {
    pub address: Redefined_Address,
    pub data:    LibmdbxLogData,
}

#[derive(
    Debug,
    Clone,
    Redefined,
    serde::Serialize,
    serde::Deserialize,
    rkyv::Serialize,
    rkyv::Deserialize,
    rkyv::Archive,
)]
#[redefined(LogData)]
#[redefined_attr(to_source = "LogData::new_unchecked(self.topics.into_iter().map(Into::into).\
                              collect(), self.data.into())")]
pub struct LibmdbxLogData {
    #[redefined_attr(func = "src.topics().into_iter().map(|t| t.clone().into()).collect()")]
    pub topics: Vec<Redefined_FixedBytes<32>>,
    pub data:   Redefined_Alloy_Bytes,
}

#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    serde::Serialize,
    serde::Deserialize,
    rkyv::Serialize,
    rkyv::Deserialize,
    rkyv::Archive,
    Redefined,
)]
#[redefined(TransactionTrace)]
pub struct LibmdbxTransactionTrace {
    pub action:        LibmdbxAction,
    pub error:         Option<String>,
    pub result:        Option<LibmdbxTraceOutput>,
    pub subtraces:     usize,
    pub trace_address: Vec<usize>,
}

#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    serde::Serialize,
    serde::Deserialize,
    rkyv::Serialize,
    rkyv::Deserialize,
    rkyv::Archive,
    Redefined,
)]
#[redefined(Action)]
pub enum LibmdbxAction {
    Call(LibmdbxCallAction),
    Create(LibmdbxCreateAction),
    Selfdestruct(LibmdbxSelfdestructAction),
    Reward(LibmdbxRewardAction),
}

#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    serde::Serialize,
    serde::Deserialize,
    rkyv::Serialize,
    rkyv::Deserialize,
    rkyv::Archive,
    Redefined,
)]
#[redefined(CallAction)]
pub struct LibmdbxCallAction {
    pub from:      Redefined_Address,
    pub call_type: LibmdbxCallType,
    pub gas:       Redefined_U64,
    pub input:     Redefined_Alloy_Bytes,
    pub to:        Redefined_Address,
    pub value:     Redefined_U256,
}

#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    serde::Serialize,
    serde::Deserialize,
    rkyv::Serialize,
    rkyv::Deserialize,
    rkyv::Archive,
    Redefined,
)]
#[redefined(CreateAction)]
pub struct LibmdbxCreateAction {
    pub from:  Redefined_Address,
    pub gas:   Redefined_U64,
    pub init:  Redefined_Alloy_Bytes,
    pub value: Redefined_U256,
}

#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    serde::Serialize,
    serde::Deserialize,
    rkyv::Serialize,
    rkyv::Deserialize,
    rkyv::Archive,
    Redefined,
)]
#[redefined(SelfdestructAction)]
pub struct LibmdbxSelfdestructAction {
    pub address:        Redefined_Address,
    pub balance:        Redefined_U256,
    pub refund_address: Redefined_Address,
}

#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    serde::Serialize,
    serde::Deserialize,
    rkyv::Serialize,
    rkyv::Deserialize,
    rkyv::Archive,
    Redefined,
)]
#[redefined(RewardAction)]
pub struct LibmdbxRewardAction {
    pub author:      Redefined_Address,
    pub reward_type: LibmdbxRewardType,
    pub value:       Redefined_U256,
}

#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    serde::Serialize,
    serde::Deserialize,
    rkyv::Serialize,
    rkyv::Deserialize,
    rkyv::Archive,
    Redefined,
)]
#[redefined(RewardType)]
pub enum LibmdbxRewardType {
    Block,
    Uncle,
}

#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    serde::Serialize,
    serde::Deserialize,
    rkyv::Serialize,
    rkyv::Deserialize,
    rkyv::Archive,
    Redefined,
)]
#[redefined(CallType)]
pub enum LibmdbxCallType {
    None,
    Call,
    CallCode,
    DelegateCall,
    StaticCall,
}

#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    serde::Serialize,
    serde::Deserialize,
    rkyv::Serialize,
    rkyv::Deserialize,
    rkyv::Archive,
    Redefined,
)]
#[redefined(TraceOutput)]
pub enum LibmdbxTraceOutput {
    Call(LibmdbxCallOutput),
    Create(LibmdbxCreateOutput),
}

#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    serde::Serialize,
    serde::Deserialize,
    rkyv::Serialize,
    rkyv::Deserialize,
    rkyv::Archive,
    Redefined,
)]
#[redefined(CallOutput)]
pub struct LibmdbxCallOutput {
    pub gas_used: Redefined_U64,
    pub output:   Redefined_Alloy_Bytes,
}

#[derive(
    Clone,
    Debug,
    Eq,
    PartialEq,
    serde::Serialize,
    serde::Deserialize,
    rkyv::Serialize,
    rkyv::Deserialize,
    rkyv::Archive,
    Redefined,
)]
#[redefined(CreateOutput)]
pub struct LibmdbxCreateOutput {
    pub address:  Redefined_Address,
    pub code:     Redefined_Alloy_Bytes,
    pub gas_used: Redefined_U64,
}

//  Libmdbx
