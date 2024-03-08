use std::{sync::Arc, vec};

use alloy_sol_macro::sol;
use alloy_sol_types::SolCall;
use brontes_types::traits::TracingProvider;
use futures::join;
use reth_rpc_types::{request::TransactionInput, TransactionRequest};

use super::{IErc20, UniswapV3Pool};
use crate::{errors::AmmError, protocols::make_call_request};
sol!(
    IGetUniswapV3PoolDataBatchRequest,
    "./src/protocols/uniswap_v3/batch_request/GetUniswapV3PoolDataBatchRequestABI.json"
);
sol!(
    IGetUniswapV3TickDataBatchRequest,
    "./src/protocols/uniswap_v3/batch_request/GetUniswapV3TickDataBatchRequestABI.json"
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

    let req = TransactionRequest {
        to: None,
        input: TransactionInput::new(bytecode.into()),
        ..Default::default()
    };

    let res = middleware
        .eth_call(req, block_number.map(|i| i.into()), None, None)
        .await
        .map_err(|e| eyre::eyre!("v3 data fetch call failed, err={}", e))?;

    let mut return_data = data_constructorCall::abi_decode_returns(&res, false)?;
    *pool = populate_pool_data_from_tokens(pool.to_owned(), return_data._0.remove(0));

    let (r0, r1) = join!(
        make_call_request(
            IErc20::balanceOfCall::new((pool.address,)),
            &middleware,
            pool.token_a,
            block_number,
        ),
        make_call_request(
            IErc20::balanceOfCall::new((pool.address,)),
            &middleware,
            pool.token_b,
            block_number,
        )
    );

    pool.reserve_0 = r0?._0;
    pool.reserve_1 = r1?._0;

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

    let req = TransactionRequest {
        to: None,
        input: TransactionInput::new(bytecode.into()),
        ..Default::default()
    };

    let res = middleware
        .eth_call(req, block_number.map(Into::into), None, None)
        .await
        .unwrap();

    let return_data = tick_constructorCall::abi_decode_returns(&res, false).unwrap();

    Ok((return_data._0, return_data._1))
}
