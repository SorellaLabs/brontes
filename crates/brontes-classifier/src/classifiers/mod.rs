use brontes_macros::{action_dispatch, discovery_dispatch};
use futures::StreamExt;
pub mod erc20;

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

pub mod fluid;
pub use fluid::*;

pub mod gmx;
pub use gmx::*;

pub mod dolomite;
pub use dolomite::*;

pub mod maker;
pub use maker::*;

pub mod zerox;
pub use zerox::*;

pub mod cowswap;
pub use cowswap::*;

pub mod oneinch;
pub use oneinch::*;

pub mod clipper;
pub use clipper::*;

pub mod dodo;
pub use dodo::*;

pub mod maverick_v2;
pub use maverick_v2::*;
pub mod pendle_v2;
pub use pendle_v2::*;

pub mod camelot;
pub use camelot::*;

pub mod dexalot;
pub use dexalot::*;

pub mod lfj;
pub use lfj::*;

discovery_dispatch!(
    DiscoveryClassifier,
    SushiSwapV2Discovery,
    SushiSwapV3Discovery,
    UniswapV2Discovery,
    UniswapV3Discovery,
    PancakeSwapV3Discovery,
    PancakeSwapV2Discovery,
    // CurveV1MetaDiscovery,
    CurveV2PlainDiscovery0,
    CurveV2PlainDiscovery1,
    CurveV2PlainDiscovery2,
    CurveV2MetaDiscovery0,
    CurveV2MetaDiscovery1,
    // CurvecrvUSDPlainDiscovery0,
    // CurvecrvUSDPlainDiscovery1,
    // CurvecrvUSDPlainDiscovery2,
    // CurvecrvUSDMetaDiscovery0,
    // CurvecrvUSDMetaDiscovery1,
    CurveCryptoSwapDiscovery,
    CurveTriCryptoDiscovery,
    // BalancerV1CoreDiscovery,
    // BalancerV1SmartPoolDiscovery,
    CamelotV2Discovery,
    CamelotV3Discovery,
    FluidDexFactoryDiscovery
);

action_dispatch!(
    ProtocolClassifier,
    UniswapV2SwapCall,
    UniswapV2MintCall,
    UniswapV2BurnCall,
    SushiSwapV2SwapCall,
    SushiSwapV2MintCall,
    SushiSwapV2BurnCall,
    PancakeSwapV2SwapCall,
    PancakeSwapV2MintCall,
    PancakeSwapV2BurnCall,
    UniswapV3SwapCall,
    UniswapV3MintCall,
    UniswapV3BurnCall,
    UniswapV3CollectCall,
    SushiSwapV3SwapCall,
    SushiSwapV3MintCall,
    SushiSwapV3BurnCall,
    SushiSwapV3CollectCall,
    PancakeSwapV3SwapCall,
    PancakeSwapV3MintCall,
    PancakeSwapV3BurnCall,
    PancakeSwapV3CollectCall,
    UniswapXExecuteCall,
    UniswapXExecuteBatchCall,
    UniswapXExecuteBatchWithCallbackCall,
    UniswapXExecuteWithCallbackCall,
    CurveBasePool2ExchangeCall,
    CurveBasePool3ExchangeCall,
    CurveBasePool4ExchangeCall,
    CurveBasePool2Add_liquidityCall,
    CurveBasePool3Add_liquidityCall,
    CurveBasePool4Add_liquidityCall,
    CurveBasePool2Remove_liquidityCall,
    CurveBasePool2Remove_liquidity_imbalanceCall,
    // CurveBasePool2remove_liquidity_one_coinCall,
    //CurveBasePool2remove_liquidity_one_coinCallLido,
    CurveBasePool3Remove_liquidityCall,
    CurveBasePool3Remove_liquidity_imbalanceCall,
    CurveBasePool3Remove_liquidity_one_coinCall,
    CurveBasePool4Remove_liquidityCall,
    CurveBasePool4Remove_liquidity_imbalanceCall,
    CurveV1MetapoolImplExchange_0Call,
    CurveV1MetapoolImplExchange_1Call,
    CurveV1MetapoolImplExchange_underlying_0Call,
    CurveV1MetapoolImplExchange_underlying_1Call,
    CurveV1MetapoolImplAdd_liquidity_0Call,
    CurveV1MetapoolImplAdd_liquidity_1Call,
    CurveV1MetapoolImplRemove_liquidity_0Call,
    CurveV1MetapoolImplRemove_liquidity_1Call,
    CurveV1MetapoolImplRemove_liquidity_imbalance_0Call,
    CurveV1MetapoolImplRemove_liquidity_imbalance_1Call,
    CurveV1MetapoolImplRemove_liquidity_one_coin_0Call,
    CurveV1MetapoolImplRemove_liquidity_one_coin_1Call,
    CurveV2MetapoolImplExchange_0Call,
    CurveV2MetapoolImplExchange_1Call,
    CurveV2MetapoolImplAdd_liquidity_0Call,
    CurveV2MetapoolImplAdd_liquidity_1Call,
    CurveV2MetapoolImplExchange_underlying_0Call,
    CurveV2MetapoolImplExchange_underlying_1Call,
    CurveV2MetapoolImplRemove_liquidity_0Call,
    CurveV2MetapoolImplRemove_liquidity_1Call,
    CurveV2MetapoolImplRemove_liquidity_imbalance_0Call,
    CurveV2MetapoolImplRemove_liquidity_imbalance_1Call,
    CurveV2MetapoolImplRemove_liquidity_one_coin_0Call,
    CurveV2MetapoolImplRemove_liquidity_one_coin_1Call,
    CurveV2PlainPoolImplExchange_0Call,
    CurveV2PlainPoolImplExchange_1Call,
    CurveV2PlainPoolImplAdd_liquidity_0Call,
    CurveV2PlainPoolImplAdd_liquidity_1Call,
    CurveV2PlainPoolImplRemove_liquidity_0Call,
    CurveV2PlainPoolImplRemove_liquidity_1Call,
    CurveV2PlainPoolImplRemove_liquidity_imbalance_0Call,
    CurveV2PlainPoolImplRemove_liquidity_imbalance_1Call,
    CurveV2PlainPoolImplRemove_liquidity_one_coin_0Call,
    CurveV2PlainPoolImplRemove_liquidity_one_coin_1Call,
    MakerPSMBuyGemCall,
    MakerPSMSellGemCall,
    MakerDssFlashFlashLoanCall,
    AaveV2LiquidationCallCall,
    AaveV3LiquidationCallCall,
    AaveV2FlashLoanCall,
    AaveV3FlashLoanCall,
    AaveV3FlashLoanSimpleCall,
    BalancerV1SwapExactAmountInCall,
    BalancerV1SwapExactAmountOutCall,
    BalancerV1BindCall,
    BalancerV2OnSwap_0Call,
    BalancerV2OnSwap_1Call,
    BalancerV2FlashLoanCall,
    BalancerV2JoinPoolCall,
    BalancerV2ExitPoolCall,
    BalancerV2RegisterTokensCall,
    CompoundV2LiquidateBorrowCall,
    CompoundV2Initialize_0Call,
    CompoundV2Initialize_1Call,
    OneInchV5SwapCall,
    OneInchV5ClipperSwapCall,
    OneInchV5ClipperSwapToCall,
    OneInchV5ClipperSwapToWithPermitCall,
    OneInchV5UnoswapToCall,
    OneInchV5UnoswapToWithPermitCall,
    OneInchV5UniswapV3SwapToCall,
    OneInchV5UniswapV3SwapToWithPermitCall,
    OneInchFusionSettleOrdersCall,
    ClipperExchangeSwapCall,
    ClipperExchangeSellEthForTokenCall,
    ClipperExchangeSellTokenForEthCall,
    ClipperExchangeTransmitAndSwapCall,
    ClipperExchangeTransmitAndSellTokenForEthCall,
    CowswapSettleCall,
    CowswapSwapCall,
    ZeroXSellToUniswapCall,
    ZeroXSellEthForTokenToUniswapV3Call,
    ZeroXSellTokenForEthToUniswapV3Call,
    ZeroXSellTokenForTokenToUniswapV3Call,
    ZeroXTransformERC20Call,
    ZeroXSellToPancakeSwapCall,
    ZeroXFillOtcOrderCall,
    ZeroXFillOtcOrderForEthCall,
    ZeroXFillOtcOrderWithEthCall,
    ZeroXFillTakerSignedOtcOrderCall,
    ZeroXFillTakerSignedOtcOrderForEthCall,
    ZeroXBatchFillTakerSignedOtcOrdersCall,
    ZeroXSellToLiquidityProviderCall,
    ZeroXMultiplexBatchSellEthForTokenCall,
    ZeroXMultiplexBatchSellTokenForEthCall,
    ZeroXMultiplexBatchSellTokenForTokenCall,
    ZeroXMultiplexMultiHopSellEthForTokenCall,
    ZeroXMultiplexMultiHopSellTokenForEthCall,
    ZeroXMultiplexMultiHopSellTokenForTokenCall,
    ZeroXFillLimitOrderCall,
    ZeroXFillRfqOrderCall,
    ZeroXFillOrKillLimitOrderCall,
    ZeroXFillOrKillRfqOrderCall,
    DodoCreateDODOVendingMachineCall,
    DodoCreateDODOStablePoolCall,
    DodoInitDODOPrivatePoolCall,
    DodoBuySharesCall,
    DodoSellSharesCall,
    DodoSellBaseCall,
    DodoSellQuoteCall,
    DodoFlashLoanCall,
    MaverickV2SwapCall,
    MaverickV2AddLiquidityCall,
    MaverickV2RemoveLiquidityCall,
    MaverickV2Create_0Call,
    MaverickV2Create_1Call,
    MaverickV2CreatePermissioned_0Call,
    MaverickV2CreatePermissioned_1Call,
    CamelotV2Swap_0Call,
    CamelotV2Swap_1Call,
    CamelotV3SwapCall,
    PendleV2CreateNewMarketCall,
    PendleV2CreateYieldContractCall,
    CompoundV3SetConfigurationCall,
    DexalotPartialSwapCall,
    DexalotSimpleSwapCall,
    LFJCreateLBPairCall,
    LFJSwapCall,
    LFJMintCall,
    LFJBurnCall,
    LFJCollectProtocolFeesCall
);
