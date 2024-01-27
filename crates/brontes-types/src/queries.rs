use std::sync::Arc;

use alloy_primitives::Address;
use alloy_sol_types::SolCall;
use reth_rpc_types::{CallInput, CallRequest};

use crate::traits::TracingProvider;

pub async fn make_call_request<C: SolCall, T: TracingProvider>(
    call: C,
    provider: &Arc<T>,
    to: Address,
    block: Option<u64>,
) -> eyre::Result<C::Return> {
    let encoded = call.abi_encode();
    let req =
        CallRequest { to: Some(to), input: CallInput::new(encoded.into()), ..Default::default() };

    let res = provider
        .eth_call(req, block.map(Into::into), None, None)
        .await?;

    Ok(C::abi_decode_returns(&res, false)?)
}
