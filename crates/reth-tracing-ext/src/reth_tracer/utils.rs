//! Util functions for revm related ops

use alloy_primitives::hex;
use alloy_sol_types::{ContractError, GenericRevertReason};
use revm::{
    interpreter::{opcode, OpCode},
    primitives::SpecId,
};

/// creates the memory data in 32byte chunks
/// see <https://github.com/ethereum/go-ethereum/blob/366d2169fbc0e0f803b68c042b77b6b480836dbc/eth/tracers/logger/logger.go#L450-L452>
#[inline]
pub(crate) fn convert_memory(data: &[u8]) -> Vec<String> {
    let mut memory = Vec::with_capacity(data.len().div_ceil(32));
    for idx in (0..data.len()).step_by(32) {
        let len = std::cmp::min(idx + 32, data.len());
        memory.push(hex::encode(&data[idx..len]));
    }
    memory
}

/// Get the gas used, accounting for refunds
#[inline]
pub(crate) fn gas_used(spec: SpecId, spent: u64, refunded: u64) -> u64 {
    let refund_quotient = if SpecId::enabled(spec, SpecId::LONDON) { 5 } else { 2 };
    spent - (refunded).min(spent / refund_quotient)
}

/// Returns a non empty revert reason if the output is a revert/error.
#[inline]
pub(crate) fn maybe_revert_reason(output: &[u8]) -> Option<String> {
    let reason = match GenericRevertReason::decode(output)? {
        GenericRevertReason::ContractError(err) => {
            match err {
                // return the raw revert reason and don't use the revert's display message
                ContractError::Revert(revert) => revert.reason,
                err => err.to_string(),
            }
        }
        GenericRevertReason::RawString(err) => err,
    };
    if reason.is_empty() {
        None
    } else {
        Some(reason)
    }
}

/// Returns the number of items pushed on the stack by a given opcode.
/// This used to determine how many stack etries to put in the `push` element
/// in a parity vmTrace.
/// The value is obvious for most opcodes, but SWAP* and DUP* are a bit weird,
/// and we handle those as they are handled in parity vmtraces.
/// For reference: <https://github.com/ledgerwatch/erigon/blob/9b74cf0384385817459f88250d1d9c459a18eab1/turbo/jsonrpc/trace_adhoc.go#L451>
pub(crate) const fn stack_push_count(step_op: OpCode) -> usize {
    let step_op = step_op.get();
    match step_op {
        opcode::PUSH0..=opcode::PUSH32 => 1,
        opcode::SWAP1..=opcode::SWAP16 => (step_op - opcode::SWAP1) as usize + 2,
        opcode::DUP1..=opcode::DUP16 => (step_op - opcode::DUP1) as usize + 2,
        opcode::CALLDATALOAD
        | opcode::SLOAD
        | opcode::MLOAD
        | opcode::CALLDATASIZE
        | opcode::LT
        | opcode::GT
        | opcode::DIV
        | opcode::SDIV
        | opcode::SAR
        | opcode::AND
        | opcode::EQ
        | opcode::CALLVALUE
        | opcode::ISZERO
        | opcode::ADD
        | opcode::EXP
        | opcode::CALLER
        | opcode::KECCAK256
        | opcode::SUB
        | opcode::ADDRESS
        | opcode::GAS
        | opcode::MUL
        | opcode::RETURNDATASIZE
        | opcode::NOT
        | opcode::SHR
        | opcode::SHL
        | opcode::EXTCODESIZE
        | opcode::SLT
        | opcode::OR
        | opcode::NUMBER
        | opcode::PC
        | opcode::TIMESTAMP
        | opcode::BALANCE
        | opcode::SELFBALANCE
        | opcode::MULMOD
        | opcode::ADDMOD
        | opcode::BASEFEE
        | opcode::BLOCKHASH
        | opcode::BYTE
        | opcode::XOR
        | opcode::ORIGIN
        | opcode::CODESIZE
        | opcode::MOD
        | opcode::SIGNEXTEND
        | opcode::GASLIMIT
        | opcode::DIFFICULTY
        | opcode::SGT
        | opcode::GASPRICE
        | opcode::MSIZE
        | opcode::EXTCODEHASH
        | opcode::SMOD
        | opcode::CHAINID
        | opcode::COINBASE => 1,
        _ => 0,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TraceStyle {
    /// Parity style tracer
    Parity,
    /// Geth style tracer
    #[allow(dead_code)]
    Geth,
}

impl TraceStyle {
    /// Returns true if this is a parity style tracer.
    pub(crate) const fn is_parity(self) -> bool {
        matches!(self, Self::Parity)
    }
}
