use std::{sync::Arc, vec};

use alloy_primitives::FixedBytes;
use alloy_sol_macro::sol;
use alloy_sol_types::{SolCall, SolValue};
use brontes_types::traits::TracingProvider;
use reth_rpc_types::{CallInput, CallRequest};

use super::UniswapV3Pool;
use crate::{errors::AmmError, AutomatedMarketMaker};
sol!(
    IGetUniswapV3PoolDataBatchRequest,
    "./src/exchanges/uniswap_v3/batch_request/GetUniswapV3PoolDataBatchRequestABI.json"
);
sol!(
    IGetUniswapV3TickDataBatchRequest,
    "./src/exchanges/uniswap_v3/batch_request/GetUniswapV3TickDataBatchRequestABI.json"
);
sol!(
    ISyncUniswapV3PoolBatchRequest,
    "./src/exchanges/uniswap_v3/batch_request/SyncUniswapV3PoolBatchRequestABI.json"
);

sol!(
    struct PoolData {
        address tokenA;
        uint8 tokenADecimals;
        address tokenB;
        uint8 tokenBDecimals;
        uint128 liquidity;
        uint160 sqrtPrice;
        int24 tick;
        int24 tickSpacing;
        uint24 fee;
        int128 liquidityNet;
    }
    struct TickData {
        bool initialized;
        int24 tick;
        int128 liquidityNet;
    }

    function data_constructor(
        address[] memory pools
    ) returns(PoolData[]);

    function tick_constructor(
        address pool,
        bool zeroForOne,
        int24 currentTick,
        uint16 numTicks,
        int24 tickSpacing
    ) returns (TickData[], uint64);
);

fn populate_pool_data_from_tokens(mut pool: UniswapV3Pool, tokens: PoolData) -> UniswapV3Pool {
    pool.token_a = tokens.tokenA;
    pool.token_a_decimals = tokens.tokenADecimals;
    pool.token_b = tokens.tokenB;
    pool.token_b_decimals = tokens.tokenBDecimals;
    pool.liquidity = tokens.liquidity;
    pool.sqrt_price = tokens.sqrtPrice;
    pool.tick = tokens.tick;
    pool.tick_spacing = tokens.tickSpacing;
    pool.fee = tokens.fee;

    pool
}

pub async fn get_v3_pool_data_batch_request<M: TracingProvider>(
    pool: &mut UniswapV3Pool,
    block_number: Option<u64>,
    middleware: Arc<M>,
) -> Result<(), AmmError> {
    let mut bytecode = IGetUniswapV3PoolDataBatchRequest::BYTECODE.to_vec();
    data_constructorCall::new((vec![pool.address],)).abi_encode_raw(&mut bytecode);

    let req =
        CallRequest { to: None, input: CallInput::new(bytecode.into()), ..Default::default() };

    let res = middleware
        .eth_call(req, block_number.map(|i| i.into()), None, None)
        .await
        .unwrap();

    let mut return_data = data_constructorCall::abi_decode_returns(&*res, false).unwrap();
    *pool = populate_pool_data_from_tokens(pool.to_owned(), return_data._0.remove(0));

    Ok(())
}

pub async fn get_uniswap_v3_tick_data_batch_request<M: TracingProvider>(
    pool: &UniswapV3Pool,
    tick_start: i32,
    zero_for_one: bool,
    num_ticks: u16,
    block_number: Option<u64>,
    middleware: Arc<M>,
) -> Result<(Vec<TickData>, u64), AmmError> {
    let mut bytecode = IGetUniswapV3TickDataBatchRequest::BYTECODE.to_vec();
    tick_constructorCall::new((
        pool.address,
        zero_for_one,
        tick_start,
        num_ticks,
        pool.tick_spacing,
    ))
    .abi_encode_raw(&mut bytecode);

    let req =
        CallRequest { to: None, input: CallInput::new(bytecode.into()), ..Default::default() };

    let res = middleware
        .eth_call(req, block_number.map(Into::into), None, None)
        .await
        .unwrap();

    let return_data = tick_constructorCall::abi_decode_returns(&*res, false).unwrap();

    Ok((return_data._0, return_data._1))
}
