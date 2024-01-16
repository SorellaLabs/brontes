pub mod curve;
pub mod uniswap;

use std::sync::Arc;

use alloy_primitives::Log;
use brontes_pricing::types::DiscoveredPool;
use brontes_types::{exchanges::StaticBindingsDb, traits::TracingProvider};
pub use curve::*;
pub use uniswap::*;

pub trait FactoryDecoder<T: TracingProvider> {
    fn get_signature(&self) -> [u8; 32];

    #[allow(unused)]
    async fn decode_new_pool(
        &self,
        node_handle: Arc<T>,
        protocol: StaticBindingsDb,
        logs: &Vec<Log>,
    ) -> Vec<DiscoveredPool>;
}

pub trait FactoryDecoderDispatch<T: TracingProvider>: Sync + Send {
    async fn dispatch(
        sig: [u8; 32],
        node_handle: Arc<T>,
        protocol: StaticBindingsDb,
        logs: &Vec<Log>,
    ) -> Vec<DiscoveredPool>;
}
