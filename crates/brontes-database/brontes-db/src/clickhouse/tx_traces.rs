use serde::Serialize;

#[derive(Serialize, ::clickhouse::Row)]
pub struct TxTraceRow {
    pub block_number:    u64,
    pub tx_hash:         String,
    pub traces:          Vec<u8>,
    pub gas_used:        u64,
    pub effective_price: u64,
    pub tx_index:        u64,
    pub is_success:      bool,
}

pub type MetaTuple = (u64, String, Option<String>, u64, Vec<u64>);
pub type DecodedTuple = (u64, String, Vec<(String, String, String)>, Vec<(String, String, String)>);
pub type LogTuple = (u64, u64, String, Vec<String>, String);
pub type CreateActionTuple = (u64, String, u64, String, [u8; 32]);
pub type CallActionTuple = (u64, String, String, u64, String, String, [u8; 32]);
pub type SelfDestructTuple = (u64, String, [u8; 32], String);
pub type RewardTuple = (u64, String, String, [u8; 32]);
pub type CallOutputTuple = (u64, u64, String);
pub type CreateOutputTuple = (u64, String, String, u64);

#[derive(Serialize)]
pub struct TxTraceTuple(
    pub u64,
    pub (
        Vec<MetaTuple>,
        Vec<DecodedTuple>,
        Vec<LogTuple>,
        Vec<CreateActionTuple>,
        Vec<CallActionTuple>,
        Vec<SelfDestructTuple>,
        Vec<RewardTuple>,
        Vec<CallOutputTuple>,
        Vec<CreateOutputTuple>,
    ),
    pub String,
    pub u128,
    pub u128,
    pub u64,
    pub bool,
);
