pub mod curve;
pub mod sushiswap;
pub mod uniswap;

pub use curve::*;
pub use sushiswap::*;
pub use uniswap::*;

#[async_trait::async_trait]
pub trait FactoryDecoder {
    fn get_signature(&self) -> [u8; 32];

    #[allow(unused)]
    async fn decode_new_pool<'a>(
        &self,
        node_handle: &'a dyn EthProvider,
        protocol: ContractProtocol,
        logs: &Vec<Log>,
    ) -> Vec<PoolDB>;
}

#[async_trait::async_trait]
pub trait ActionCollection: Sync + Send {
    async fn dispatch(
        sig: [u8; 32],
        node_handle: &dyn EthProvider,
        protocol: ContractProtocol,
        logs: &Vec<Log>,
    ) -> Vec<PoolDB>;
}
