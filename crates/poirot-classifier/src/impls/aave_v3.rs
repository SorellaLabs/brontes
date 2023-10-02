use alloy_sol_types::{SolCall, SolEvent};

use crate::IntoAction;

#[derive(Debug)]
pub struct AaveV3Supply;

impl IntoAction for AaveV3Supply {
    fn get_signature(&self) -> [u8; 4] {
        todo!()
    }

    fn decode_trace_data(
        &self,
        index: u64,
        data: poirot_core::StaticReturnBindings,
        return_data: reth_primitives::Bytes,
        address: reth_primitives::Address,
        logs: &Vec<reth_rpc_types::Log>,
    ) -> poirot_types::normalized_actions::Actions {
        todo!()
    }
}
