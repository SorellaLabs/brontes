use std::sync::Arc;

use alloy_primitives::Address;
use alloy_sol_types::SolCall;
use reth_rpc_types::{request::TransactionInput, TransactionRequest};

use crate::traits::TracingProvider;

pub async fn make_call_request<C: SolCall, T: TracingProvider>(
    call: C,
    provider: &Arc<T>,
    to: Address,
    block: Option<u64>,
) -> eyre::Result<C::Return> {
    let encoded = call.abi_encode();
    let req = TransactionRequest {
        to: Some(alloy_primitives::TxKind::Call(to)),
        input: TransactionInput::new(encoded.into()),
        ..Default::default()
    };

    let res = provider
        .eth_call(req, block.map(Into::into), None, None)
        .await?;

    Ok(C::abi_decode_returns(&res, false)?)
}

alloy_sol_macro::sol!(
    function yeet(address babies) returns (bool is_dead);
);
