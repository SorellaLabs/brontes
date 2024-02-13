use brontes_macros::{action_dispatch, discovery_dispatch};
pub mod transfer;

pub mod uniswap;
pub use uniswap::*;

pub mod sushiswap;
pub use sushiswap::*;

pub mod curve;
pub use curve::*;

pub mod balancer;
pub use balancer::*;

pub mod aave;
pub use aave::*;

pub mod pancakeswap;
pub use pancakeswap::*;

pub mod maker;
pub use maker::*;

discovery_dispatch!(
    DiscoveryProtocols,
    SushiSwapV2Discovery,
    SushiSwapV3Discovery,
    UniswapV2Discovery,
    UniswapV3Discovery,
    PancakeSwapV3Discovery,
    PancakeSwapV2Discovery,
    CurveV1MetapoolBaseDiscovery,
    CurveV1MetapoolMetaDiscovery,
    CurveV2MetapoolBaseDiscovery,
    CurveV2MetapoolMetaDiscovery0,
    CurveV2MetapoolMetaDiscovery1,
    CurveV2MetapoolPlainDiscovery0,
    CurveV2MetapoolPlainDiscovery1,
    CurveV2MetapoolPlainDiscovery2
);

action_dispatch!(
    ProtocolClassifications,
    UniswapV2swapCall,
    UniswapV2mintCall,
    UniswapV2burnCall,
    SushiSwapV2swapCall,
    SushiSwapV2mintCall,
    SushiSwapV2burnCall,
    PancakeSwapV2swapCall,
    PancakeSwapV2mintCall,
    PancakeSwapV2burnCall,
    UniswapV3swapCall,
    UniswapV3mintCall,
    UniswapV3burnCall,
    UniswapV3collectCall,
    SushiSwapV3swapCall,
    SushiSwapV3mintCall,
    SushiSwapV3burnCall,
    SushiSwapV3collectCall,
    PancakeSwapV3swapCall,
    PancakeSwapV3mintCall,
    PancakeSwapV3burnCall,
    PancakeSwapV3collectCall,
    UniswapXexecuteCall,
    UniswapXexecuteBatchCall,
    UniswapXexecuteBatchWithCallbackCall,
    UniswapXexecuteWithCallbackCall,
    CurveCryptoSwapexchange_0Call,
    CurveCryptoSwapexchange_1Call,
    CurveCryptoSwapexchange_2Call,
    CurveCryptoSwapexchange_underlying_0Call,
    MakerPSMbuyGemCall,
    MakerPSMsellGemCall,
    AaveV2liquidationCallCall,
    AaveV3liquidationCallCall,
    AaveV2flashLoanCall,
    AaveV3flashLoanCall,
    AaveV3flashLoanSimpleCall,
    BalancerV1swapExactAmountInCall,
    BalancerV1swapExactAmountOutCall
);
