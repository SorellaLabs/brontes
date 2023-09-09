use reth_primitives::{Address, Bytes, Log};

use poirot_core::StaticReturnBindings;

pub mod classifer;
pub use classifer::*;

mod impls;
pub use impls::*;

use poirot_types::normalized_actions::Actions;

pub trait IntoAction: Send + Sync {
    fn decode_trace_data(
        &self,
        data: StaticReturnBindings,
        return_data: Bytes,
        address: Address,
        logs: &Vec<Log>,
    ) -> Actions;
}
