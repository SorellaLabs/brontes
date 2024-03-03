use crate::TracingInspectorLocal;
use alloy_primitives::{Address, Bytes, Log, U256};
use revm::{
    inspectors::GasInspector,
    interpreter::{
        opcode, CallInputs, CallOutcome, CallScheme, CreateInputs, CreateOutcome,
        InstructionResult, Interpreter, InterpreterResult, OpCode,
    },
    primitives::SpecId,
    Database, EvmContext, Inspector, JournalEntry,
};

#[derive(Clone, Debug)]
pub struct BrontesTracingInspector {
    /// Configures what and how the inspector records traces.
    config: TracingInspectorConfig,
    /// Records all call traces
    traces: CallTraceArena,
    /// Tracks active calls
    trace_stack: Vec<usize>,
    /// Tracks active steps
    step_stack: Vec<StackStep>,
    /// Tracks the return value of the last call
    last_call_return_data: Option<Bytes>,
    /// The gas inspector used to track remaining gas.
    gas_inspector: GasInspector,
    /// The spec id of the EVM.
    ///
    /// This is filled during execution.
    spec_id: Option<SpecId>,
}

