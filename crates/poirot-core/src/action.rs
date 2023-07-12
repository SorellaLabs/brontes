use ethers::{
    abi::{Abi, Function, Token},
    types::H160,
};

use reth_rpc_types::trace::parity::LocalizedTransactionTrace;

/// An [`Action`] is the lowest level parsing type, analogous to a lexeme in compiler design.
#[derive(Debug, Clone)]
pub struct Action {
    /// Function that was called.
    function: Function,
    /// Vector of inputs to the function.
    inputs: Vec<Token>,
    /// If it is a known protocol, the type.
    protocol: Option<ProtocolType>,
    /// The underlying trace the call came from.
    trace: LocalizedTransactionTrace,
}

/// A type of protocol.
/// TODO: Add more, and in addition add detection.
#[derive(Debug, Clone)]
pub enum ProtocolType {
    Uniswap,
    Curve,
}

impl Action {
    /// Public constructor function to instantiate an [`Action`].
    pub fn new(function: Function, inputs: Vec<Token>, trace: LocalizedTransactionTrace) -> Self {
        Self { function, inputs, protocol: None, trace }
    }
}
