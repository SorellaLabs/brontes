use std::fmt::Debug;

use brontes_core::StaticReturnBindings;
use reth_primitives::{Address, Bytes, H160};
use reth_rpc_types::Log;

pub mod classifier;
pub use classifier::*;

mod impls;
use brontes_types::normalized_actions::Actions;
pub use impls::*;

include!(concat!(env!("OUT_DIR"), "/token_mappings.rs"));

pub trait IntoAction: Debug + Send + Sync {
    fn get_signature(&self) -> [u8; 4];

    fn decode_trace_data(
        &self,
        index: u64,
        data: StaticReturnBindings,
        return_data: Bytes,
        from_address: Address,
        target_address: Address,
        logs: &Vec<Log>,
    ) -> Actions;
}

pub trait ActionCollection {
    fn dispatch(
        &self,
        sig: [u8; 4],
        index: u64,
        data: StaticReturnBindings,
        return_data: Bytes,
        from_address: Address,
        target_address: Address,
        logs: &Vec<Log>,
    ) -> Actions;
}
