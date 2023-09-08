use reth_primitives::Bytes;

pub mod classifer;
pub use classifer::*;

mod impls;
pub use impls::*;

use poirot_types::normalized_actions::Actions;

pub trait IntoAction {
    fn decode_trace_data(&self, calldata: Bytes, return_data: Bytes) -> Actions;
}
