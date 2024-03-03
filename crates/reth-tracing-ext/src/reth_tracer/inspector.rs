use revm_inspectors::tracing::TracingInspectorConfig;
use types::{CallTrace, CallTraceStep, CallKind, CallTraceNode, LogCallOrder, RecordedMemory, StorageChange, StorageChangeReason};
use arena::{PushTraceKind, CallTraceArena};
use utils::{gas_used, stack_push_count};
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
use super::types;
use super::arena;
use super::utils;

#[cfg(feature = "js-tracer")]
pub mod js;

/// An inspector that collects call traces.
///
/// This [Inspector] can be hooked into revm's EVM which then calls the inspector
/// functions, such as [Inspector::call] or [Inspector::call_end].
///
/// The [TracingInspector] keeps track of everything by:
///   1. start tracking steps/calls on [Inspector::step] and [Inspector::call]
///   2. complete steps/calls on [Inspector::step_end] and [Inspector::call_end]
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

// === impl TracingInspector ===

impl BrontesTracingInspector {
    /// Returns a new instance for the given config
    pub fn new(config: TracingInspectorConfig) -> Self {
        Self {
            config,
            traces: Default::default(),
            trace_stack: vec![],
            step_stack: vec![],
            last_call_return_data: None,
            gas_inspector: Default::default(),
            spec_id: None,
        }
    }

    /// Returns the config of the inspector.
    pub const fn config(&self) -> &TracingInspectorConfig {
        &self.config
    }

    /// Gets a reference to the recorded call traces.
    pub const fn get_traces(&self) -> &CallTraceArena {
        &self.traces
    }

    /// Gets a mutable reference to the recorded call traces.
    pub fn get_traces_mut(&mut self) -> &mut CallTraceArena {
        &mut self.traces
    }

    /// Manually the gas used of the root trace.
    ///
    /// This is useful if the root trace's gasUsed should mirror the actual gas used by the
    /// transaction.
    ///
    /// This allows setting it manually by consuming the execution result's gas for example.
    #[inline]
    pub fn set_transaction_gas_used(&mut self, gas_used: u64) {
        if let Some(node) = self.traces.arena.first_mut() {
            node.trace.gas_used = gas_used;
        }
    }

    /// Convenience function for [ParityTraceBuilder::set_transaction_gas_used] that consumes the
    /// type.
    #[inline]
    pub fn with_transaction_gas_used(mut self, gas_used: u64) -> Self {
        self.set_transaction_gas_used(gas_used);
        self
    }

    /// Returns true if we're no longer in the context of the root call.
    fn is_deep(&self) -> bool {
        // the root call will always be the first entry in the trace stack
        !self.trace_stack.is_empty()
    }

    /// Returns true if this a call to a precompile contract.
    ///
    /// Returns true if the `to` address is a precompile contract and the value is zero.
    #[inline]
    fn is_precompile_call<DB: Database>(
        &self,
        context: &EvmContext<DB>,
        to: &Address,
        value: U256,
    ) -> bool {
        if context.precompiles.contains(to) {
            // only if this is _not_ the root call
            return self.is_deep() && value.is_zero();
        }
        false
    }

    /// Returns the currently active call trace.
    ///
    /// This will be the last call trace pushed to the stack: the call we entered most recently.
    #[track_caller]
    #[inline]
    fn active_trace(&self) -> Option<&CallTraceNode> {
        self.trace_stack.last().map(|idx| &self.traces.arena[*idx])
    }

    /// Returns the last trace [CallTrace] index from the stack.
    ///
    /// This will be the currently active call trace.
    ///
    /// # Panics
    ///
    /// If no [CallTrace] was pushed
    #[track_caller]
    #[inline]
    fn last_trace_idx(&self) -> usize {
        self.trace_stack.last().copied().expect("can't start step without starting a trace first")
    }

    /// _Removes_ the last trace [CallTrace] index from the stack.
    ///
    /// # Panics
    ///
    /// If no [CallTrace] was pushed
    #[track_caller]
    #[inline]
    fn pop_trace_idx(&mut self) -> usize {
        self.trace_stack.pop().expect("more traces were filled than started")
    }

    /// Starts tracking a new trace.
    ///
    /// Invoked on [Inspector::call].
    #[allow(clippy::too_many_arguments)]
    fn start_trace_on_call<DB: Database>(
        &mut self,
        context: &EvmContext<DB>,
        address: Address,
        input_data: Bytes,
        value: U256,
        kind: CallKind,
        caller: Address,
        mut gas_limit: u64,
        maybe_precompile: Option<bool>,
    ) {
        // This will only be true if the inspector is configured to exclude precompiles and the call
        // is to a precompile
        let push_kind = if maybe_precompile.unwrap_or(false) {
            // We don't want to track precompiles
            PushTraceKind::PushOnly
        } else {
            PushTraceKind::PushAndAttachToParent
        };

        if self.trace_stack.is_empty() {
            // this is the root call which should get the original gas limit of the transaction,
            // because initialization costs are already subtracted from gas_limit
            // For the root call this value should use the transaction's gas limit
            // See <https://github.com/paradigmxyz/reth/issues/3678> and <https://github.com/ethereum/go-ethereum/pull/27029>
            gas_limit = context.env.tx.gas_limit;

            // we set the spec id here because we only need to do this once and this condition is
            // hit exactly once
            self.spec_id = Some(context.spec_id());
        }

        self.trace_stack.push(self.traces.push_trace(
            0,
            push_kind,
            CallTrace {
                depth: context.journaled_state.depth() as usize,
                address,
                kind,
                data: input_data,
                value,
                status: InstructionResult::Continue,
                caller,
                maybe_precompile,
                gas_limit,
                ..Default::default()
            },
        ));
    }

    /// Fills the current trace with the outcome of a call.
    ///
    /// Invoked on [Inspector::call_end].
    ///
    /// # Panics
    ///
    /// This expects an existing trace [Self::start_trace_on_call]
    fn fill_trace_on_call_end<DB: Database>(
        &mut self,
        context: &mut EvmContext<DB>,
        result: InterpreterResult,
        created_address: Option<Address>,
    ) {
        let InterpreterResult { result, output, gas } = result;

        let trace_idx = self.pop_trace_idx();
        let trace = &mut self.traces.arena[trace_idx].trace;

        if trace_idx == 0 {
            // this is the root call which should get the gas used of the transaction
            // refunds are applied after execution, which is when the root call ends
            trace.gas_used = gas_used(context.spec_id(), gas.spend(), gas.refunded() as u64);
        } else {
            trace.gas_used = gas.spend();
        }

        trace.status = result;
        trace.success = trace.status.is_ok();
        trace.output = output.clone();

        self.last_call_return_data = Some(output);

        if let Some(address) = created_address {
            // A new contract was created via CREATE
            trace.address = address;
        }
    }

    /// Starts tracking a step
    ///
    /// Invoked on [Inspector::step]
    ///
    /// # Panics
    ///
    /// This expects an existing [CallTrace], in other words, this panics if not within the context
    /// of a call.
    fn start_step<DB: Database>(&mut self, interp: &mut Interpreter, context: &mut EvmContext<DB>) {
        let trace_idx = self.last_trace_idx();
        let trace = &mut self.traces.arena[trace_idx];

        self.step_stack.push(StackStep { trace_idx, step_idx: trace.trace.steps.len() });

        let memory = self
            .config
            .record_memory_snapshots
            .then(|| RecordedMemory::new(interp.shared_memory.context_memory().to_vec()))
            .unwrap_or_default();
        let stack = if self.config.record_stack_snapshots.is_full() {
            Some(interp.stack.data().clone())
        } else {
            None
        };

        let op = OpCode::new(interp.current_opcode())
            .or_else(|| {
                // if the opcode is invalid, we'll use the invalid opcode to represent it because
                // this is invoked before the opcode is executed, the evm will eventually return a
                // `Halt` with invalid/unknown opcode as result
                let invalid_opcode = 0xfe;
                OpCode::new(invalid_opcode)
            })
            .expect("is valid opcode;");

        trace.trace.steps.push(CallTraceStep {
            depth: context.journaled_state.depth(),
            pc: interp.program_counter(),
            op,
            contract: interp.contract.address,
            stack,
            push_stack: None,
            memory_size: memory.len(),
            memory,
            gas_remaining: self.gas_inspector.gas_remaining(),
            gas_refund_counter: interp.gas.refunded() as u64,

            // fields will be populated end of call
            gas_cost: 0,
            storage_change: None,
            status: InstructionResult::Continue,
        });
    }

    /// Fills the current trace with the output of a step.
    ///
    /// Invoked on [Inspector::step_end].
    fn fill_step_on_step_end<DB: Database>(
        &mut self,
        interp: &Interpreter,
        context: &EvmContext<DB>,
    ) {
        let StackStep { trace_idx, step_idx } =
            self.step_stack.pop().expect("can't fill step without starting a step first");
        let step = &mut self.traces.arena[trace_idx].trace.steps[step_idx];

        if self.config.record_stack_snapshots.is_pushes() {
            let num_pushed = stack_push_count(step.op);
            let start = interp.stack.len() - num_pushed;
            step.push_stack = Some(interp.stack.data()[start..].to_vec());
        }

        if self.config.record_memory_snapshots {
            // resize memory so opcodes that allocated memory is correctly displayed
            if interp.shared_memory.len() > step.memory.len() {
                step.memory.resize(interp.shared_memory.len());
            }
        }
        if self.config.record_state_diff {
            let op = step.op.get();

            let journal_entry = context
                .journaled_state
                .journal
                .last()
                // This should always work because revm initializes it as `vec![vec![]]`
                // See [JournaledState::new](revm::JournaledState)
                .expect("exists; initialized with vec")
                .last();

            step.storage_change = match (op, journal_entry) {
                (
                    opcode::SLOAD | opcode::SSTORE,
                    Some(JournalEntry::StorageChange { address, key, had_value }),
                ) => {
                    // SAFETY: (Address,key) exists if part if StorageChange
                    let value = context.journaled_state.state[address].storage[key].present_value();
                    let reason = match op {
                        opcode::SLOAD => StorageChangeReason::SLOAD,
                        opcode::SSTORE => StorageChangeReason::SSTORE,
                        _ => unreachable!(),
                    };
                    let change = StorageChange { key: *key, value, had_value: *had_value, reason };
                    Some(change)
                }
                _ => None,
            };
        }

        // The gas cost is the difference between the recorded gas remaining at the start of the
        // step the remaining gas here, at the end of the step.
        // TODO: Figure out why this can overflow. https://github.com/paradigmxyz/evm-inspectors/pull/38
        step.gas_cost = step.gas_remaining.saturating_sub(self.gas_inspector.gas_remaining());

        // set the status
        step.status = interp.instruction_result;
    }
}

impl<DB> Inspector<DB> for BrontesTracingInspector
where
    DB: Database,
{
    #[inline]
    fn initialize_interp(&mut self, interp: &mut Interpreter, context: &mut EvmContext<DB>) {
        self.gas_inspector.initialize_interp(interp, context)
    }

    fn step(&mut self, interp: &mut Interpreter, context: &mut EvmContext<DB>) {
        if self.config.record_steps {
            self.gas_inspector.step(interp, context);
            self.start_step(interp, context);
        }
    }

    fn step_end(&mut self, interp: &mut Interpreter, context: &mut EvmContext<DB>) {
        if self.config.record_steps {
            self.gas_inspector.step_end(interp, context);
            self.fill_step_on_step_end(interp, context);
        }
    }

    fn log(&mut self, context: &mut EvmContext<DB>, log: &Log) {
        self.gas_inspector.log(context, log);

        let trace_idx = self.last_trace_idx();
        let trace = &mut self.traces.arena[trace_idx];

        if self.config.record_logs {
            trace.ordering.push(LogCallOrder::Log(trace.logs.len()));
            trace.logs.push(log.data.clone());
        }
    }

    fn call(
        &mut self,
        context: &mut EvmContext<DB>,
        inputs: &mut CallInputs,
    ) -> Option<CallOutcome> {
        self.gas_inspector.call(context, inputs);

        // determine correct `from` and `to` based on the call scheme
        let (from, to) = match inputs.context.scheme {
            CallScheme::DelegateCall | CallScheme::CallCode => {
                (inputs.context.address, inputs.context.code_address)
            }
            _ => (inputs.context.caller, inputs.context.address),
        };

        let value = if matches!(inputs.context.scheme, CallScheme::DelegateCall) {
            // for delegate calls we need to use the value of the top trace
            if let Some(parent) = self.active_trace() {
                parent.trace.value
            } else {
                inputs.transfer.value
            }
        } else {
            inputs.transfer.value
        };

        // if calls to precompiles should be excluded, check whether this is a call to a precompile
        let maybe_precompile = self
            .config
            .exclude_precompile_calls
            .then(|| self.is_precompile_call(context, &to, value));

        self.start_trace_on_call(
            context,
            to,
            inputs.input.clone(),
            value,
            inputs.context.scheme.into(),
            from,
            inputs.gas_limit,
            maybe_precompile,
        );

        None
    }

    fn call_end(
        &mut self,
        context: &mut EvmContext<DB>,
        inputs: &CallInputs,
        outcome: CallOutcome,
    ) -> CallOutcome {
        let outcome = self.gas_inspector.call_end(context, inputs, outcome);

        self.fill_trace_on_call_end(context, outcome.result.clone(), None);

        outcome
    }

    fn create(
        &mut self,
        context: &mut EvmContext<DB>,
        inputs: &mut CreateInputs,
    ) -> Option<CreateOutcome> {
        self.gas_inspector.create(context, inputs);

        let _ = context.load_account(inputs.caller);
        let nonce = context.journaled_state.account(inputs.caller).info.nonce;
        self.start_trace_on_call(
            context,
            inputs.created_address(nonce),
            inputs.init_code.clone(),
            inputs.value,
            inputs.scheme.into(),
            inputs.caller,
            inputs.gas_limit,
            Some(false),
        );

        None
    }

    /// Called when a contract has been created.
    ///
    /// InstructionResulting anything other than the values passed to this function (`(ret,
    /// remaining_gas, address, out)`) will alter the result of the create.
    fn create_end(
        &mut self,
        context: &mut EvmContext<DB>,
        inputs: &CreateInputs,
        outcome: CreateOutcome,
    ) -> CreateOutcome {
        let outcome = self.gas_inspector.create_end(context, inputs, outcome);

        // get the code of the created contract
        let _code = outcome
            .address
            .and_then(|address| {
                context
                    .journaled_state
                    .account(address)
                    .info
                    .code
                    .as_ref()
                    .map(|code| code.bytes()[..code.len()].to_vec())
            })
            .unwrap_or_default();

        self.fill_trace_on_call_end(context, outcome.result.clone(), outcome.address);

        outcome
    }

    fn selfdestruct(&mut self, _contract: Address, target: Address, _value: U256) {
        let trace_idx = self.last_trace_idx();
        let trace = &mut self.traces.arena[trace_idx].trace;
        trace.selfdestruct_refund_target = Some(target)
    }
}

#[derive(Clone, Copy, Debug)]
struct StackStep {
    trace_idx: usize,
    step_idx: usize,
}

