use alloy_dyn_abi::DynSolValue;
use reth_primitives::{Address, H256, U256};
use reth_rpc_types::trace::parity::{CreateAction, RewardAction, SelfdestructAction};

use crate::{
    normalized_actions::Actions,
    tree::{Node, Root},
};

// A structured trace is a tx trace that has been decoded & parsed with its subsequent traces
#[derive(Debug, Clone)]

pub enum StructuredTrace {
    CALL(CallAction),
    CREATE(CreateAction),
    SELFDESTRUCT(SelfdestructAction),
    REWARD(RewardAction),
}

impl StructuredTrace {
    pub fn get_from_addr(&self) -> Address {
        match self {
            StructuredTrace::CALL(c) => c.from,
            StructuredTrace::CREATE(c) => c.from,
            StructuredTrace::SELFDESTRUCT(c) => c.address, // check this
            StructuredTrace::REWARD(c) => c.author,
        }
    }

    pub fn get_call_len(&self) -> usize {
        match self {
            StructuredTrace::CALL(c) => c.trace_address.len(),
            StructuredTrace::CREATE(_) => panic!("SHOULD NEVER REACH THIS"),
            StructuredTrace::SELFDESTRUCT(_) => panic!("SHOULD NEVER REACH THIS"),
            StructuredTrace::REWARD(_) => panic!("SHOULD NEVER REACH THIS"),
        }
    }
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

impl Into<Root<Actions>> for TxTrace {
    fn into(self) -> Root<Actions> {
        let node = Node {
            inner: vec![],
            frozen: false,
            subactions: vec![],
            address: self.trace[0].get_from_addr(),
            data: Actions::None,
        };
        let mut root = Root { head: node, tx_hash: self.tx_hash };

        let traces = self.trace[1..].to_vec();
        for trace in traces {
            let node = Node {
                inner: vec![],
                frozen: false,
                subactions: vec![],
                address: trace.get_from_addr(),
                data: Actions::None,
            };
            root.insert(node.address, node);
        }

        root
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
