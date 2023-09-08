use reth_primitives::Bytes;

pub mod classifer;
pub use classifer::Classifier;

mod impls;
pub use impls::*;

pub trait IntoAction {
    pub fn decode_trace_data(&self, calldata: Bytes, return_data: Bytes) -> Actions;
}
