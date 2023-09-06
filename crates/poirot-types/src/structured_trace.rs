use alloy_dyn_abi::DynSolValue;
use reth_primitives::{Address, H256, U256};
use reth_rpc_types::trace::parity::{CreateAction, RewardAction, SelfdestructAction};

// A structured trace is a tx trace that has been decoded & parsed with its subsequent traces
#[derive(Debug, Clone)]

pub enum StructuredTrace {
    CALL(CallAction),
    CREATE(CreateAction),
    SELFDESTRUCT(SelfdestructAction),
    REWARD(RewardAction),
}

pub struct TxTrace {
    pub trace: Vec<StructuredTrace>,
    pub tx_hash: H256,
    pub tx_index: usize,
}

impl TxTrace {
    pub fn new(trace: Vec<StructuredTrace>, tx_hash: H256, tx_index: usize) -> Self {
        Self { trace, tx_hash, tx_index }
    }
}

#[derive(Debug, Clone)]
pub struct CallAction {
    pub from: Address,
    pub to: Address,
    pub value: U256,

    /// Name of the function that has been called.
    pub function_name: String,
    /// Vector of inputs to the function.
    pub inputs: Option<DynSolValue>,
    //
    pub trace_address: Vec<usize>,
}

impl CallAction {
    /// Public constructor function to instantiate an [`Action`].
    pub fn new(
        from: Address,
        to: Address,
        value: U256,
        function_name: String,
        inputs: Option<DynSolValue>,
        trace_address: Vec<usize>,
    ) -> Self {
        Self { from, to, value, function_name, inputs, trace_address }
    }
}
