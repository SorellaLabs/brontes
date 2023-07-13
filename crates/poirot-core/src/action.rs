use alloy_dyn_abi::DynSolValue;
use reth_rpc_types::trace::parity::LocalizedTransactionTrace;

/// An [`Action`] is the lowest level parsing type, analogous to a lexeme in compiler design.
#[derive(Debug, Clone)]

pub struct Action {
    /// Name of the function that has been called.
    pub function_name: String,
    /// Vector of inputs to the function.
    pub inputs: Option<DynSolValue>,
    /// If it is a known protocol, the type.
    pub protocol: Option<ProtocolType>,
    /// The underlying trace the call came from.
    pub trace: LocalizedTransactionTrace,
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
    pub fn new(
        function_name: String,
        inputs: Option<DynSolValue>,
        trace: LocalizedTransactionTrace,
    ) -> Self {
        Self { function_name, inputs, protocol: None, trace }
    }
}
