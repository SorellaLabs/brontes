use std::{fmt::Debug, sync::Arc};

use alloy_primitives::{Address, Bytes};
use brontes_database::libmdbx::LibmdbxReader;
use brontes_pricing::types::{DiscoveredPool, PoolUpdate};
use brontes_types::{structured_trace::CallFrameInfo, traits::TracingProvider};
use futures::Future;

pub mod tree_builder;
pub use tree_builder::Classifier;

#[cfg(feature = "tests")]
pub mod test_utils;

mod classifiers;
use alloy_sol_types::sol;
use brontes_types::normalized_actions::Actions;
pub use classifiers::*;

// Actions
sol!(UniswapV2, "./classifier-abis/UniswapV2.json");
sol!(SushiSwapV2, "./classifier-abis/SushiSwapV2.json");
sol!(UniswapV3, "./classifier-abis/UniswapV3.json");
sol!(SushiSwapV3, "./classifier-abis/SushiSwapV3.json");
sol!(PancakeSwapV3, "./classifier-abis/PancakeSwapV3.json");
sol!(CurveCryptoSwap, "./classifier-abis/CurveCryptoSwap.json");
sol!(BalancerV1, "./classifier-abis/BalancerV1Pool.json");
sol!(AaveV2, "./classifier-abis/AaveV2Pool.json");
sol!(AaveV3, "./classifier-abis/AaveV3Pool.json");
sol!(UniswapX, "./classifier-abis/UniswapXExclusiveDutchOrderReactor.json");
sol!(MakerPSM, "./classifier-abis/MakerPSM.json");

// Discovery
sol!(UniswapV2Factory, "./classifier-abis/UniswapV2Factory.json");
sol!(UniswapV3Factory, "./classifier-abis/UniswapV3Factory.json");
sol!(CurveV1MetapoolFactory, "./classifier-abis/CurveMetapoolFactoryV1.json");
sol!(CurveV2MetapoolFactory, "./classifier-abis/CurveMetapoolFactoryV2.json");
sol!(CurvecrvUSDFactory, "./classifier-abis/CurveCRVUSDFactory.json");
sol!(CurveCryptoSwapFactory, "./classifier-abis/CurveCryptoSwapFactory.json");
sol!(CurveTriCryptoFactory, "./classifier-abis/CurveTriCryptoFactory.json");
sol! {
    event Transfer(address indexed from, address indexed to, uint256 value);
    function name() public view returns (string);
    function symbol() public view returns (string);
    function decimals() public view returns (uint8);
    function totalSupply() public view returns (uint256);
}

pub trait ActionCollection: Sync + Send {
    fn dispatch<DB: LibmdbxReader>(
        &self,
        call_info: CallFrameInfo<'_>,
        db_tx: &DB,
        block: u64,
        tx_idx: u64,
    ) -> Option<(PoolUpdate, Actions)>;
}

pub trait IntoAction: Debug + Send + Sync {
    fn decode_trace_data<DB: LibmdbxReader>(
        &self,
        call_info: CallFrameInfo<'_>,
        db_tx: &DB,
    ) -> eyre::Result<Actions>;
}

pub trait FactoryDecoder {
    fn decode_new_pool<T: TracingProvider>(
        &self,
        tracer: Arc<T>,
        deployed_address: Address,
        parent_calldata: Bytes,
    ) -> impl Future<Output = Vec<DiscoveredPool>> + Send;
}

pub trait FactoryDecoderDispatch: Sync + Send {
    fn dispatch<T: TracingProvider>(
        &self,
        tracer: Arc<T>,
        factory: Address,
        deployed_address: Address,
        parent_calldata: Bytes,
    ) -> impl Future<Output = Vec<DiscoveredPool>> + Send;
}
