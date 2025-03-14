//! Types for representing call trace items.

pub use alloy_primitives::Log;
use alloy_primitives::{Address, Bytes, FixedBytes, LogData, U256};
use alloy_rpc_types_trace::{
    geth::{CallFrame, CallLogFrame},
    parity::{
        Action, ActionType, CallAction, CallOutput, CallType, CreateAction, CreateOutput,
        CreationMethod, SelfdestructAction, TraceOutput, TransactionTrace,
    },
};
use revm::{
    bytecode::opcode::OpCode,
    interpreter::{CallScheme, CreateScheme, InstructionResult},
};

use super::{
    config::TraceStyle,
    utils::{self, convert_memory},
};

/// Decoded call data.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DecodedCallData {
    /// The function signature.
    pub signature: String,
    /// The function arguments.
    pub args: Vec<String>,
}

/// Additional decoded data enhancing the [CallTrace].
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DecodedCallTrace {
    /// Optional decoded label for the call.
    pub label: Option<String>,
    /// Optional decoded return data.
    pub return_data: Option<String>,
    /// Optional decoded call data.
    pub call_data: Option<DecodedCallData>,
}

/// A trace of a call with optional decoded data.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CallTrace {
    /// The depth of the call.
    pub depth: usize,
    /// Whether the call was successful.
    pub success: bool,
    /// The caller address.
    pub caller: Address,
    /// The target address of this call.
    ///
    /// This is:
    /// - [`CallKind::Call`] and alike: the callee, the address of the contract
    ///   being called
    /// - [`CallKind::Create`] and alike: the address of the created contract
    pub address: Address,
    /// Whether this is a call to a precompile.
    ///
    /// Note: This is optional because not all tracers make use of this.
    pub maybe_precompile: Option<bool>,
    /// The address of the selfdestructed contract.
    pub selfdestruct_address: Option<Address>,
    /// Holds the target for the selfdestruct refund target.
    ///
    /// This is only `Some` if a selfdestruct was executed and the call is
    /// executed before the Cancun hardfork.
    ///
    /// See [`is_selfdestruct`](Self::is_selfdestruct) for more information.
    pub selfdestruct_refund_target: Option<Address>,
    /// The value transferred on a selfdestruct.
    ///
    /// This is only `Some` if a selfdestruct was executed and the call is
    /// executed before the Cancun hardfork.
    ///
    /// See [`is_selfdestruct`](Self::is_selfdestruct) for more information.
    pub selfdestruct_transferred_value: Option<U256>,
    /// The kind of call.
    pub kind: CallKind,
    /// The value transferred in the call.
    pub value: U256,
    /// The calldata/input, or the init code for contract creations.
    pub data: Bytes,
    /// The return data, or the runtime bytecode of the created contract.
    pub output: Bytes,
    /// The total gas cost of the call.
    pub gas_used: u64,
    /// The gas limit of the call.
    pub gas_limit: u64,
    /// The final status of the call.
    pub status: InstructionResult,
    /// Opcode-level execution steps.
    pub steps: Vec<CallTraceStep>,
    /// Optional complementary decoded call data.
    pub decoded: DecodedCallTrace,
}

impl CallTrace {
    /// Returns true if the status code is an error or revert, See
    /// [InstructionResult::Revert]
    #[inline]
    pub const fn is_error(&self) -> bool {
        !self.status.is_ok()
    }

    /// Returns true if the status code is a revert.
    #[inline]
    pub fn is_revert(&self) -> bool {
        self.status == InstructionResult::Revert
    }

    /// Returns `true` if this trace was a selfdestruct.
    ///
    /// See also `TracingInspector::selfdestruct`.
    ///
    /// We can't rely entirely on [`Self::status`] being
    /// [`InstructionResult::SelfDestruct`] because there's an edge case
    /// where a new created contract (CREATE) is immediately selfdestructed.
    ///
    /// We also can't rely entirely on `selfdestruct_refund_target` being `Some`
    /// as the `selfdestruct` inspector function will not be called after
    /// the Cancun hardfork.
    #[inline]
    pub const fn is_selfdestruct(&self) -> bool {
        matches!(self.status, InstructionResult::SelfDestruct)
            || self.selfdestruct_refund_target.is_some()
    }

    /// Returns the error message if it is an erroneous result.
    pub(crate) fn as_error_msg(&self, kind: TraceStyle) -> Option<String> {
        // See also <https://github.com/ethereum/go-ethereum/blob/34d507215951fb3f4a5983b65e127577989a6db8/eth/tracers/native/call_flat.go#L39-L55>
        self.is_error().then(|| match self.status {
            InstructionResult::Revert => {
                if kind.is_parity() { "Reverted" } else { "execution reverted" }.to_string()
            }
            InstructionResult::OutOfGas | InstructionResult::PrecompileOOG => {
                if kind.is_parity() { "Out of gas" } else { "out of gas" }.to_string()
            }
            InstructionResult::OutOfFunds => if kind.is_parity() {
                "Insufficient balance for transfer"
            } else {
                "insufficient balance for transfer"
            }
            .to_string(),
            InstructionResult::MemoryOOG => {
                if kind.is_parity() { "Out of gas" } else { "out of gas: out of memory" }
                    .to_string()
            }
            InstructionResult::MemoryLimitOOG => {
                if kind.is_parity() { "Out of gas" } else { "out of gas: reach memory limit" }
                    .to_string()
            }
            InstructionResult::InvalidOperandOOG => {
                if kind.is_parity() { "Out of gas" } else { "out of gas: invalid operand" }
                    .to_string()
            }
            InstructionResult::OpcodeNotFound => {
                if kind.is_parity() { "Bad instruction" } else { "invalid opcode" }.to_string()
            }
            InstructionResult::StackOverflow => "Out of stack".to_string(),
            InstructionResult::InvalidJump => {
                if kind.is_parity() { "Bad jump destination" } else { "invalid jump destination" }
                    .to_string()
            }
            InstructionResult::PrecompileError => {
                if kind.is_parity() { "Built-in failed" } else { "precompiled failed" }.to_string()
            }
            InstructionResult::InvalidFEOpcode => {
                if kind.is_parity() { "Bad instruction" } else { "invalid opcode: INVALID" }
                    .to_string()
            }
            // TODO(mattsse): upcoming error
            // InstructionResult::ReentrancySentryOOG => if kind.is_parity() {
            //     "Out of gas"
            // } else {
            //     "out of gas: not enough gas for reentrancy sentry"
            // }
            // .to_string(),
            status => format!("{:?}", status),
        })
    }
}

/// Additional decoded data enhancing the [CallLog].
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DecodedCallLog {
    /// The decoded event name.
    pub name: Option<String>,
    /// The decoded log parameters, a vector of the parameter name (e.g. foo)
    /// and the parameter value (e.g. 0x9d3...45ca).
    pub params: Option<Vec<(String, String)>>,
}

/// A log with optional decoded data.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CallLog {
    /// The raw log data.
    pub raw_log: LogData,
    /// Optional complementary decoded log data.
    pub decoded: DecodedCallLog,
    /// The position of the log relative to subcalls within the same trace.
    pub position: u64,
}

impl From<Log> for CallLog {
    /// Converts a [`Log`] into a [`CallLog`].
    fn from(log: Log) -> Self {
        Self {
            position: Default::default(),
            raw_log: log.data,
            decoded: DecodedCallLog { name: None, params: None },
        }
    }
}

impl CallLog {
    /// Sets the position of the log.
    #[inline]
    pub fn with_position(mut self, position: u64) -> Self {
        self.position = position;
        self
    }
}

/// A node in the arena
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CallTraceNode {
    /// Parent node index in the arena
    pub parent: Option<usize>,
    /// Children node indexes in the arena
    pub children: Vec<usize>,
    /// This node's index in the arena
    pub idx: usize,
    /// The call trace
    pub trace: CallTrace,
    /// Recorded logs, if enabled
    pub logs: Vec<CallLog>,
    /// Ordering of child calls and logs
    pub ordering: Vec<TraceMemberOrder>,
}

impl CallTraceNode {
    /// Returns the call context's execution address
    ///
    /// See `Inspector::call` impl of
    /// [TracingInspector](crate::tracing::TracingInspector)
    pub const fn execution_address(&self) -> Address {
        if self.trace.kind.is_delegate() {
            self.trace.caller
        } else {
            self.trace.address
        }
    }

    /// Returns true if this is a call to a precompile
    #[inline]
    pub fn is_precompile(&self) -> bool {
        self.trace.maybe_precompile.unwrap_or(false)
    }

    /// Returns the kind of call the trace belongs to
    #[inline]
    pub const fn kind(&self) -> CallKind {
        self.trace.kind
    }

    /// Returns the status of the call
    #[inline]
    pub const fn status(&self) -> InstructionResult {
        self.trace.status
    }

    /// Returns the call context's 4 byte selector
    pub fn selector(&self) -> Option<FixedBytes<4>> {
        (self.trace.data.len() >= 4).then(|| FixedBytes::from_slice(&self.trace.data[..4]))
    }

    /// Returns `true` if this trace was a selfdestruct.
    ///
    /// See [`CallTrace::is_selfdestruct`] for more details.
    #[inline]
    pub const fn is_selfdestruct(&self) -> bool {
        self.trace.is_selfdestruct()
    }

    /// Converts this node into a parity `TransactionTrace`
    pub fn parity_transaction_trace(&self, trace_address: Vec<usize>) -> TransactionTrace {
        let action = self.parity_action();
        let result = if self.trace.is_error() && !self.trace.is_revert() {
            // if the trace is a selfdestruct or an error that is not a revert, the result
            // is None
            None
        } else {
            Some(self.parity_trace_output())
        };
        let error = self.trace.as_error_msg(TraceStyle::Parity);
        TransactionTrace { action, error, result, trace_address, subtraces: self.children.len() }
    }

    /// Returns the `Output` for a parity trace
    pub fn parity_trace_output(&self) -> TraceOutput {
        match self.kind() {
            CallKind::Call
            | CallKind::StaticCall
            | CallKind::CallCode
            | CallKind::DelegateCall
            | CallKind::AuthCall => TraceOutput::Call(CallOutput {
                gas_used: self.trace.gas_used,
                output: self.trace.output.clone(),
            }),
            CallKind::Create | CallKind::Create2 | CallKind::EOFCreate => {
                TraceOutput::Create(CreateOutput {
                    gas_used: self.trace.gas_used,
                    code: self.trace.output.clone(),
                    address: self.trace.address,
                })
            }
        }
    }

    /// If the trace is a selfdestruct, returns the `Action` for a parity trace.
    pub fn parity_selfdestruct_action(&self) -> Option<Action> {
        self.is_selfdestruct().then(|| {
            Action::Selfdestruct(SelfdestructAction {
                address: self.trace.selfdestruct_address.unwrap_or_default(),
                refund_address: self.trace.selfdestruct_refund_target.unwrap_or_default(),
                balance: self
                    .trace
                    .selfdestruct_transferred_value
                    .unwrap_or_default(),
            })
        })
    }

    /// If the trace is a selfdestruct, returns the `CallFrame` for a geth call
    /// trace
    pub fn geth_selfdestruct_call_trace(&self) -> Option<CallFrame> {
        self.is_selfdestruct().then(|| CallFrame {
            typ: "SELFDESTRUCT".to_string(),
            from: self.trace.selfdestruct_address.unwrap_or_default(),
            to: self.trace.selfdestruct_refund_target,
            value: self.trace.selfdestruct_transferred_value,
            ..Default::default()
        })
    }

    /// If the trace is a selfdestruct, returns the `TransactionTrace` for a
    /// parity trace.
    pub fn parity_selfdestruct_trace(&self, trace_address: Vec<usize>) -> Option<TransactionTrace> {
        let trace = self.parity_selfdestruct_action()?;
        Some(TransactionTrace {
            action: trace,
            error: None,
            result: None,
            trace_address,
            subtraces: 0,
        })
    }

    /// Returns the `Action` for a parity trace.
    ///
    /// Caution: This does not include the selfdestruct action, if the trace is
    /// a selfdestruct, since those are handled in addition to the call
    /// action.
    pub fn parity_action(&self) -> Action {
        match self.kind() {
            CallKind::Call
            | CallKind::StaticCall
            | CallKind::CallCode
            | CallKind::DelegateCall
            | CallKind::AuthCall => Action::Call(CallAction {
                from: self.trace.caller,
                to: self.trace.address,
                value: self.trace.value,
                gas: self.trace.gas_limit,
                input: self.trace.data.clone(),
                call_type: self.kind().into(),
            }),
            CallKind::Create | CallKind::Create2 | CallKind::EOFCreate => {
                Action::Create(CreateAction {
                    from: self.trace.caller,
                    value: self.trace.value,
                    gas: self.trace.gas_limit,
                    init: self.trace.data.clone(),
                    creation_method: self.kind().into(),
                })
            }
        }
    }

    /// Converts this call trace into an _empty_ geth [CallFrame]
    pub fn geth_empty_call_frame(&self, include_logs: bool) -> CallFrame {
        let mut call_frame = CallFrame {
            typ: self.trace.kind.to_string(),
            from: self.trace.caller,
            to: Some(self.trace.address),
            value: Some(self.trace.value),
            gas: U256::from(self.trace.gas_limit),
            gas_used: U256::from(self.trace.gas_used),
            input: self.trace.data.clone(),
            output: (!self.trace.output.is_empty()).then(|| self.trace.output.clone()),
            error: None,
            revert_reason: None,
            calls: Default::default(),
            logs: Default::default(),
        };

        if self.trace.kind.is_static_call() {
            // STATICCALL frames don't have a value
            call_frame.value = None;
        }

        // we need to populate error and revert reason
        if !self.trace.success {
            if self.kind().is_any_create() {
                call_frame.to = None;
            }

            if !self.status().is_revert() {
                call_frame.gas_used = U256::from(self.trace.gas_limit);
                call_frame.output = None;
            }

            call_frame.revert_reason = utils::maybe_revert_reason(self.trace.output.as_ref());

            // Note: regular calltracer uses geth errors, only flatCallTracer uses parity errors: <https://github.com/ethereum/go-ethereum/blob/a9523b6428238a762e1a1e55e46ead47630c3a23/eth/tracers/native/call_flat.go#L226>
            call_frame.error = self.trace.as_error_msg(TraceStyle::Geth);
        }

        if include_logs && !self.logs.is_empty() {
            call_frame.logs = self
                .logs
                .iter()
                .map(|log| CallLogFrame {
                    address: Some(self.execution_address()),
                    topics: Some(log.raw_log.topics().to_vec()),
                    data: Some(log.raw_log.data.clone()),
                    position: Some(log.position),
                })
                .collect();
        }

        call_frame
    }
}

/// A unified representation of a call.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CallKind {
    /// Represents a regular call.
    #[default]
    Call,
    /// Represents a static call.
    StaticCall,
    /// Represents a call code operation.
    CallCode,
    /// Represents a delegate call.
    DelegateCall,
    /// Represents an authorized call.
    AuthCall,
    /// Represents a contract creation operation.
    Create,
    /// Represents a contract creation operation using the CREATE2 opcode.
    Create2,
    /// Represents an EOF contract creation operation.
    EOFCreate,
}

impl CallKind {
    /// Returns the string representation of the call kind.
    pub const fn to_str(self) -> &'static str {
        match self {
            Self::Call => "CALL",
            Self::StaticCall => "STATICCALL",
            Self::CallCode => "CALLCODE",
            Self::DelegateCall => "DELEGATECALL",
            Self::AuthCall => "AUTHCALL",
            Self::Create => "CREATE",
            Self::Create2 => "CREATE2",
            Self::EOFCreate => "EOF_CREATE",
        }
    }

    /// Returns true if the call is a create
    #[inline]
    pub const fn is_any_create(&self) -> bool {
        matches!(self, Self::Create | Self::Create2 | Self::EOFCreate)
    }

    /// Returns true if the call is a delegate of some sorts
    #[inline]
    pub const fn is_delegate(&self) -> bool {
        matches!(self, Self::DelegateCall | Self::CallCode)
    }

    /// Returns true if the call is [CallKind::StaticCall].
    #[inline]
    pub const fn is_static_call(&self) -> bool {
        matches!(self, Self::StaticCall)
    }

    /// Returns true if the call is [CallKind::AuthCall].
    #[inline]
    pub const fn is_auth_call(&self) -> bool {
        matches!(self, Self::AuthCall)
    }
}

impl From<CallKind> for CreationMethod {
    fn from(kind: CallKind) -> CreationMethod {
        match kind {
            CallKind::Create => CreationMethod::Create,
            CallKind::Create2 => CreationMethod::Create2,
            CallKind::EOFCreate => CreationMethod::EofCreate,
            _ => CreationMethod::None,
        }
    }
}

impl core::fmt::Display for CallKind {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.to_str())
    }
}

impl From<CallScheme> for CallKind {
    fn from(scheme: CallScheme) -> Self {
        match scheme {
            CallScheme::Call | CallScheme::ExtCall => Self::Call,
            CallScheme::StaticCall | CallScheme::ExtStaticCall => Self::StaticCall,
            CallScheme::DelegateCall | CallScheme::ExtDelegateCall => Self::DelegateCall,
            CallScheme::CallCode => Self::CallCode,
        }
    }
}

impl From<CreateScheme> for CallKind {
    fn from(create: CreateScheme) -> Self {
        match create {
            CreateScheme::Create => Self::Create,
            CreateScheme::Create2 { .. } => Self::Create2,
        }
    }
}

impl From<CallKind> for ActionType {
    fn from(kind: CallKind) -> Self {
        match kind {
            CallKind::Call
            | CallKind::StaticCall
            | CallKind::DelegateCall
            | CallKind::CallCode
            | CallKind::AuthCall => Self::Call,
            CallKind::Create | CallKind::Create2 | CallKind::EOFCreate => Self::Create,
        }
    }
}

impl From<CallKind> for CallType {
    fn from(ty: CallKind) -> Self {
        match ty {
            CallKind::Call => Self::Call,
            CallKind::StaticCall => Self::StaticCall,
            CallKind::CallCode => Self::CallCode,
            CallKind::DelegateCall => Self::DelegateCall,
            CallKind::Create | CallKind::Create2 | CallKind::EOFCreate => Self::None,
            CallKind::AuthCall => Self::AuthCall,
        }
    }
}

/// Ordering enum for calls, logs and steps
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TraceMemberOrder {
    /// Contains the index of the corresponding log
    Log(usize),
    /// Contains the index of the corresponding trace node
    Call(usize),
    /// Contains the index of the corresponding step, if those are being traced
    Step(usize),
}

/// Represents a decoded internal function call.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DecodedInternalCall {
    /// Name of the internal function.
    pub func_name: String,
    /// Input arguments of the internal function.
    pub args: Option<Vec<String>>,
    /// Optional decoded return data.
    pub return_data: Option<Vec<String>>,
}

/// Represents a decoded trace step. Currently two formats are supported.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum DecodedTraceStep {
    /// Decoded internal function call. Displayed similarly to external calls.
    ///
    /// Keeps decoded internal call data and an index of the step where the
    /// internal call execution ends.
    InternalCall(DecodedInternalCall, usize),
    /// Arbitrary line representing the step. Might be used for displaying
    /// individual opcodes.
    Line(String),
}

/// Represents a tracked call step during execution
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CallTraceStep {
    // Fields filled in `step`
    /// Call depth
    pub depth: u64,
    /// Program counter before step execution
    pub pc: usize,
    /// Code section index before step execution
    pub code_section_idx: usize,
    /// Opcode to be executed
    pub op: OpCode,
    /// Current contract address
    pub contract: Address,
    /// Stack before step execution
    pub stack: Option<Vec<U256>>,
    /// The new stack items placed by this step if any
    pub push_stack: Option<Vec<U256>>,
    /// Memory before step execution.
    ///
    /// This will be `None` only if memory capture is disabled.
    pub memory: Option<RecordedMemory>,
    /// Returndata before step execution
    pub returndata: Bytes,
    /// Remaining gas before step execution
    pub gas_remaining: u64,
    /// Gas refund counter before step execution
    pub gas_refund_counter: u64,
    /// Total gas used before step execution
    pub gas_used: u64,
    // Fields filled in `step_end`
    /// Gas cost of step execution
    pub gas_cost: u64,
    /// Change of the contract state after step execution (effect of the
    /// SLOAD/SSTORE instructions)
    pub storage_change: Option<StorageChange>,
    /// Final status of the step
    ///
    /// This is set after the step was executed.
    pub status: InstructionResult,
    /// Immediate bytes of the step
    pub immediate_bytes: Option<Bytes>,
    /// Optional complementary decoded step data.
    pub decoded: Option<DecodedTraceStep>,
}

/// Represents the source of a storage change - e.g., whether it came
/// from an SSTORE or SLOAD instruction.
#[allow(clippy::upper_case_acronyms)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum StorageChangeReason {
    /// SLOAD opcode
    SLOAD,
    /// SSTORE opcode
    SSTORE,
}

/// Represents a storage change during execution.
///
/// This maps to evm internals:
/// [JournalEntry::StorageChanged](revm::JournalEntry::StorageChanged)
///
/// It is used to track both storage change and warm load of a storage slot. For
/// warm load in regard to EIP-2929 AccessList had_value will be None.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StorageChange {
    /// key of the storage slot
    pub key: U256,
    /// Current value of the storage slot
    pub value: U256,
    /// The previous value of the storage slot, if any
    pub had_value: Option<U256>,
    /// How this storage was accessed
    pub reason: StorageChangeReason,
}

/// Represents the memory captured during execution
///
/// This is a wrapper around the [SharedMemory](revm::interpreter::SharedMemory)
/// context memory.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RecordedMemory(pub(crate) Bytes);

impl RecordedMemory {
    #[inline]
    pub(crate) fn new(mem: &[u8]) -> Self {
        if mem.is_empty() {
            return Self(Bytes::new());
        }

        Self(Bytes::copy_from_slice(mem))
    }

    /// Returns the memory as a byte slice
    #[inline]
    pub fn as_bytes(&self) -> &Bytes {
        &self.0
    }

    /// Returns the memory as a byte vector
    #[inline]
    pub fn into_bytes(self) -> Bytes {
        self.0
    }

    /// Returns the size of the memory.
    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns whether the memory is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Formats memory data into a list of 32-byte hex-encoded chunks.
    ///
    /// See: <https://github.com/ethereum/go-ethereum/blob/366d2169fbc0e0f803b68c042b77b6b480836dbc/eth/tracers/logger/logger.go#L450-L452>
    #[inline]
    pub fn memory_chunks(&self) -> Vec<String> {
        convert_memory(self.as_bytes())
    }
}

impl AsRef<[u8]> for RecordedMemory {
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}
