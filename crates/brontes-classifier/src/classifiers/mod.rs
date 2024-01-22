pub mod uniswap;
pub use uniswap::*;

pub mod sushiswap;
pub use sushiswap::*;

pub mod curve;
pub use curve::*;

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
    UniV2SwapImpl,
    UniV2BurnImpl,
    UniV2MintImpl,
    UniV3SwapImpl,
    UniV3BurnImpl,
    UniV3MintImpl,
    UniV3CollectImpl,
    SushiV2SwapImpl,
    SushiV2BurnImpl,
    SushiV2MintImpl,
    SushiV3SwapImpl,
    SushiV3BurnImpl,
    SushiV3MintImpl,
    SushiV3CollectImpl,
    UniXExecuteImpl,
    CurveCryptoExchange0,
    CurveCryptoExchange1,
    CurveCryptoExchangeUnderlying,
    LiquidationCallImplV2,
    FlashloanImplV2,
    LiquidationCallImplV3,
    FlashloanImplV3,
    FlashloanSimpleImplV3
);
