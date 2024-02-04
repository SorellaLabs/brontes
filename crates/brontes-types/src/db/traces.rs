use redefined::Redefined;
use rkyv::{Archive, Deserialize as rDeserialize, Serialize as rSerialize};
use serde::{Deserialize, Serialize};

use crate::{
    implement_table_value_codecs_with_zc,
    structured_trace::{TxTrace, TxTraceRedefined},
};

#[derive(Debug, Default, Clone, Serialize, Deserialize, Redefined)]
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
