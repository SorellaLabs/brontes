use alloy_primitives::hex;
use alloy_sol_types::{ContractError, GenericRevertReason};
use revm::primitives::hardfork::SpecId;

/// Formats memory data into a list of 32-byte hex-encoded chunks.
///
/// See: <https://github.com/ethereum/go-ethereum/blob/366d2169fbc0e0f803b68c042b77b6b480836dbc/eth/tracers/logger/logger.go#L450-L452>
pub(crate) fn convert_memory(data: &[u8]) -> Vec<String> {
    let mut memory = Vec::with_capacity(data.len().div_ceil(32));
    let chunks = data.chunks_exact(32);
    let remainder = chunks.remainder();
    for chunk in chunks {
        memory.push(hex::encode(chunk));
    }
    if !remainder.is_empty() {
        let mut last_chunk = [0u8; 32];
        last_chunk[..remainder.len()].copy_from_slice(remainder);
        memory.push(hex::encode(last_chunk));
    }
    memory
}

/// Get the gas used, accounting for refunds
#[inline]
pub(crate) fn gas_used(spec: SpecId, spent: u64, refunded: u64) -> u64 {
    let refund_quotient = if SpecId::is_enabled_in(spec, SpecId::LONDON) { 5 } else { 2 };
    spent - (refunded).min(spent / refund_quotient)
}

/// Returns a non-empty revert reason if the output is a revert/error.
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

#[cfg(test)]
mod tests {
    use alloy_sol_types::{GenericContractError, SolInterface};

    use super::*;

    #[test]
    fn decode_revert_reason() {
        let err = GenericContractError::Revert("my revert".into());
        let encoded = err.abi_encode();
        let reason = maybe_revert_reason(&encoded).unwrap();
        assert_eq!(reason, "my revert");
    }

    // <https://etherscan.io/tx/0x105707c8e3b3675a8424a7b0820b271cbe394eaf4d5065b03c273298e3a81314>
    #[test]
    fn decode_revert_reason_with_error() {
        let err = hex!("08c379a000000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000024556e697377617056323a20494e53554646494349454e545f494e5055545f414d4f554e5400000000000000000000000000000000000000000000000000000080");
        let reason = maybe_revert_reason(&err[..]).unwrap();
        assert_eq!(reason, "UniswapV2: INSUFFICIENT_INPUT_AMOUNT");
    }
}
