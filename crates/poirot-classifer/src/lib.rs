use std::fmt::Debug;

use poirot_core::StaticReturnBindings;
use reth_primitives::{Address, Bytes, H160};
use reth_rpc_types::Log;

pub mod classifer;
pub use classifer::*;

mod impls;
pub use impls::*;
use poirot_types::normalized_actions::Actions;

include!(concat!(env!("OUT_DIR"), "/token_mappings.rs"));

pub trait IntoAction: Debug + Send + Sync {
    fn get_signature(&self) -> [u8; 4];

    fn decode_trace_data(
        &self,
        index: u64,
        data: StaticReturnBindings,
        return_data: Bytes,
        address: Address,
        logs: &Vec<Log>,
    ) -> Actions;
}
