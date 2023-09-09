use crate::{yoink_decoded_type, IntoAction};
use alloy_sol_types::SolCall;
use poirot_core::{
    StaticReturnBindings,
    Uniswap_V3::{swapCall, swapReturn, Uniswap_V3Calls},
};
use poirot_types::normalized_actions::Actions;
use reth_primitives::Bytes;

pub struct V3SwapImpl;

impl IntoAction for V3SwapImpl {
    fn decode_trace_data(&self, data: StaticReturnBindings, return_data: Bytes) -> Actions {
        let res = yoink_decoded_type!(data, Uniswap_V3, swapCall);

        let r = swapCall::decode_returns(&mut &return_data[..], true).unwrap();
        todo!()
    }
}
