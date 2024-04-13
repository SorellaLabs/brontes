#![allow(non_snake_case)]
#![allow(non_camel_case_types)]

use std::{
    fmt::Debug,
    sync::{Arc, OnceLock},
};

use alloy_primitives::{Address, Bytes};
use brontes_database::libmdbx::{DBWriter, LibmdbxReader};
use brontes_metrics::classifier::ClassificationMetrics;
use brontes_pricing::types::DexPriceMsg;
use brontes_types::{
    normalized_actions::pool::NormalizedNewPool, structured_trace::CallFrameInfo,
    traits::TracingProvider,
};
use futures::Future;

pub mod tree_builder;
pub use tree_builder::Classifier;
pub mod discovery_only;
pub mod multi_frame_classification;

#[cfg(feature = "tests")]
pub mod test_utils;

mod classifiers;
use alloy_sol_types::sol;
use brontes_types::normalized_actions::Action;
pub use classifiers::*;

// Actions
sol!(UniswapV2, "./classifier-abis/UniswapV2.json");
sol!(SushiSwapV2, "./classifier-abis/SushiSwapV2.json");
sol!(UniswapV3, "./classifier-abis/UniswapV3.json");
sol!(SushiSwapV3, "./classifier-abis/SushiSwapV3.json");
sol!(PancakeSwapV2, "./classifier-abis/PancakeSwapV2.json");
sol!(PancakeSwapV3, "./classifier-abis/PancakeSwapV3.json");
sol!(CurveBase2, "./classifier-abis/CurveBase2.json");
//sol!(CurveLido2, "./classifier-abis/CurveBase2Lido.json");
sol!(CurveBase3, "./classifier-abis/CurveBase3.json");
sol!(CurveBase4, "./classifier-abis/CurveBase4.json");
sol!(CurveV1MetapoolImpl, "./classifier-abis/CurveV1MetapoolImpl.json");
sol!(CurveV2MetapoolImpl, "./classifier-abis/CurveV2MetapoolImpl.json");
sol!(CurveV2PlainImpl, "./classifier-abis/CurveV2PlainImpl.json");
sol!(CurvecrvUSDPlainImpl, "./classifier-abis/CurvecrvUSDPlainImpl.json");
sol!(CurveCryptoSwap, "./classifier-abis/CurveCryptoSwap.json");
sol!(BalancerV1, "./classifier-abis/balancer/BalancerV1Pool.json");
sol!(BalancerV2Vault, "./classifier-abis/balancer/BalancerV2Vault.json");
sol!(AaveV2, "./classifier-abis/AaveV2Pool.json");
sol!(AaveV3, "./classifier-abis/AaveV3Pool.json");
sol!(UniswapX, "./classifier-abis/UniswapXExclusiveDutchOrderReactor.json");
sol!(MakerPSM, "./classifier-abis/maker/MakerPSM.json");
sol!(MakerDssFlash, "./classifier-abis/maker/MakerDssFlash.json");
sol!(CompoundV2CToken, "./classifier-abis/CompoundV2CToken.json");
sol!(OneInchAggregationRouterV5, "./classifier-abis/OneInchAggregationRouterV5.json");
sol!(OneInchFusionSettlement, "./classifier-abis/OneInchFusionSettlement.json");
sol!(ClipperExchange, "./classifier-abis/ClipperExchange.json");
sol!(CowswapGPv2Settlement, "./classifier-abis/cowswap/GPv2Settlement.json");
sol!(ZeroXUniswapFeaure, "./classifier-abis/zero-x/ZeroXUniswapFeature.json");
sol!(ZeroXUniswapV3Feature, "./classifier-abis/zero-x/ZeroXUniswapV3Feature.json");
sol!(ZeroXTransformERC20Feature, "./classifier-abis/zero-x/ZeroXTransformERC20Feature.json");
sol!(ZeroXPancakeSwapFeature, "./classifier-abis/zero-x/ZeroXPancakeSwapFeature.json");
sol!(ZeroXOtcOrdersFeature, "./classifier-abis/zero-x/ZeroXOtcOrdersFeature.json");
sol!(ZeroXLiquidityProviderFeature, "./classifier-abis/zero-x/ZeroXLiquidityProviderFeature.json");
sol!(ZeroXInterface, "./classifier-abis/zero-x/ZeroXInterface.json");
sol!(BancorNetwork, "./classifier-abis/bancor/BancorNetwork.json");

sol!(UniswapV2, "./abis/UniswapV2.json");
sol!(SushiSwapV2, "./abis/SushiSwapV2.json");
sol!(UniswapV3, "./abis/UniswapV3.json");
sol!(SushiSwapV3, "./abis/SushiSwapV3.json");
sol!(CurveCryptoSwap, "./abis/CurveCryptoSwap.json");
sol!(AaveV2, "./abis/AaveV2Pool.json");

pub trait ActionCollection: Sync + Send {
    fn dispatch<DB: LibmdbxReader + DBWriter>(
        &self,
        call_info: CallFrameInfo<'_>,
        db_tx: &DB,
        block: u64,
        tx_idx: u64,
    ) -> Option<(DexPriceMsg, Action)>;
}

pub trait IntoAction: Debug + Send + Sync {
    fn decode_call_trace<DB: LibmdbxReader + DBWriter>(
        &self,
        call_info: CallFrameInfo<'_>,
        block: u64,
        tx_idx: u64,
        db_tx: &DB,
    ) -> eyre::Result<DexPriceMsg>;
}

pub trait FactoryDiscovery {
    fn decode_create_trace<T: TracingProvider>(
        &self,
        tracer: Arc<T>,
        deployed_address: Address,
        trace_idx: u64,
        parent_calldata: Bytes,
    ) -> impl Future<Output = Vec<NormalizedNewPool>> + Send;
}

pub trait FactoryDiscoveryDispatch: Sync + Send {
    fn dispatch<T: TracingProvider>(
        &self,
        tracer: Arc<T>,
        possible_calls: Vec<(Address, Bytes)>,
        deployed_address: Address,
        trace_idx: u64,
    ) -> impl Future<Output = Vec<NormalizedNewPool>> + Send;
}
