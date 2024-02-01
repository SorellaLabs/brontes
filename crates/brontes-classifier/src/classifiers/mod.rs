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
use brontes_macros::{action_dispatch, discovery_dispatch};

discovery_dispatch!(
    DiscoveryProtocols,
    SushiSwapV2Decoder,
    SushiSwapV3Decoder,
    UniswapV2Decoder,
    UniswapV3Decoder,
    CurveV1MetapoolBaseDecoder,
    CurveV1MetapoolMetaDecoder,
    CurveV2MetapoolBaseDecoder,
    CurveV2MetapoolMetaDecoder0,
    CurveV2MetapoolMetaDecoder1,
    CurveV2MetapoolPlainDecoder0,
    CurveV2MetapoolPlainDecoder1,
    CurveV2MetapoolPlainDecoder2
);

action_dispatch!(
    ProtocolClassifications,
    UniswapV2swapCall,
    UniswapV2mintCall,
    UniswapV2burnCall,
    SushiSwapV2swapCall,
    SushiSwapV2mintCall,
    SushiSwapV2burnCall,
    UniswapV3swapCall,
    UniswapV3mintCall,
    UniswapV3burnCall,
    UniswapV3collectCall,
    SushiSwapV3swapCall,
    SushiSwapV3mintCall,
    SushiSwapV3burnCall,
    SushiSwapV3collectCall,
    UniswapXexecuteCall,
    CurveCryptoSwapexchange_0Call,
    CurveCryptoSwapexchange_1Call,
    CurveCryptoSwapexchange_2Call,
    CurveCryptoSwapexchange_underlying_0Call,
    AaveV2liquidationCallCall,
    AaveV3liquidationCallCall,
    AaveV2flashLoanCall,
    AaveV3flashLoanCall,
    AaveV3flashLoanSimpleCall,
    BalancerV1swapExactAmountInCall,
    BalancerV1swapExactAmountOutCall
);
