use reth_primitives::Bytes;

pub mod classifer;

pub trait IntoAction {
    pub fn decode_calldata_results(&self, calldata: Bytes, return_data: Bytes) -> Self;
}
