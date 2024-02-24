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

pub mod compound;
pub use compound::*;

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
    CurveV1MetaDiscovery,
    CurveV2PlainDiscovery0,
    CurveV2PlainDiscovery1,
    CurveV2PlainDiscovery2,
    CurveV2MetaDiscovery0,
    CurveV2MetaDiscovery1,
    CurvecrvUSDPlainDiscovery0,
    CurvecrvUSDPlainDiscovery1,
    CurvecrvUSDPlainDiscovery2,
    CurvecrvUSDMetaDiscovery0,
    CurvecrvUSDMetaDiscovery1,
    CurveCryptoSwapDiscovery,
    CurveTriCryptoDiscovery
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
    CurveBasePool2exchangeCall,
    CurveBasePool3exchangeCall,
    CurveBasePool4exchangeCall,
    CurveBasePool2add_liquidityCall,
    CurveBasePool3add_liquidityCall,
    CurveBasePool4add_liquidityCall,
    CurveBasePool2remove_liquidityCall,
    CurveBasePool2remove_liquidity_imbalanceCall,
    CurveBasePool2remove_liquidity_one_coinCall,
    CurveBasePool3remove_liquidityCall,
    CurveBasePool3remove_liquidity_imbalanceCall,
    CurveBasePool3remove_liquidity_one_coinCall,
    CurveBasePool4remove_liquidityCall,
    CurveBasePool4remove_liquidity_imbalanceCall,
    CurveV1MetapoolImplexchange_0Call,
    CurveV1MetapoolImplexchange_1Call,
    CurveV1MetapoolImplexchange_underlying_0Call,
    CurveV1MetapoolImplexchange_underlying_1Call,
    CurveV1MetapoolImpladd_liquidity_0Call,
    CurveV1MetapoolImpladd_liquidity_1Call,
    CurveV1MetapoolImplremove_liquidity_0Call,
    CurveV1MetapoolImplremove_liquidity_1Call,
    CurveV1MetapoolImplremove_liquidity_imbalance_0Call,
    CurveV1MetapoolImplremove_liquidity_imbalance_1Call,
    CurveV1MetapoolImplremove_liquidity_one_coin_0Call,
    CurveV1MetapoolImplremove_liquidity_one_coin_1Call,
    CurveV2MetapoolImplexchange_0Call,
    CurveV2MetapoolImplexchange_1Call,
    CurveV2MetapoolImpladd_liquidity_0Call,
    CurveV2MetapoolImpladd_liquidity_1Call,
    CurveV2MetapoolImplexchange_underlying_0Call,
    CurveV2MetapoolImplexchange_underlying_1Call,
    CurveV2MetapoolImplremove_liquidity_0Call,
    CurveV2MetapoolImplremove_liquidity_1Call,
    CurveV2MetapoolImplremove_liquidity_imbalance_0Call,
    CurveV2MetapoolImplremove_liquidity_imbalance_1Call,
    CurveV2MetapoolImplremove_liquidity_one_coin_0Call,
    CurveV2MetapoolImplremove_liquidity_one_coin_1Call,
    CurveV2PlainPoolImplexchange_0Call,
    CurveV2PlainPoolImplexchange_1Call,
    CurveV2PlainPoolImpladd_liquidity_0Call,
    CurveV2PlainPoolImpladd_liquidity_1Call,
    CurveV2PlainPoolImplremove_liquidity_0Call,
    CurveV2PlainPoolImplremove_liquidity_1Call,
    CurveV2PlainPoolImplremove_liquidity_imbalance_0Call,
    CurveV2PlainPoolImplremove_liquidity_imbalance_1Call,
    CurveV2PlainPoolImplremove_liquidity_one_coin_0Call,
    CurveV2PlainPoolImplremove_liquidity_one_coin_1Call,
    MakerPSMbuyGemCall,
    MakerPSMsellGemCall,
    AaveV2liquidationCallCall,
    AaveV3liquidationCallCall,
    AaveV2flashLoanCall,
    AaveV3flashLoanCall,
    AaveV3flashLoanSimpleCall,
    BalancerV1swapExactAmountInCall,
    BalancerV1swapExactAmountOutCall,
    CompoundV2liquidateBorrowCall,
    CompoundV2initialize_0Call,
    CompoundV2initialize_1Call
);
