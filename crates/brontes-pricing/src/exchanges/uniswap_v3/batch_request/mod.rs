use std::{sync::Arc, vec};

use alloy_primitives::FixedBytes;
use alloy_sol_macro::sol;
use alloy_sol_types::{SolCall, SolValue};
use brontes_types::traits::TracingProvider;
use futures::join;
use reth_rpc_types::{CallInput, CallRequest};

use super::{IUniswapV3Pool::*, UniswapV3Pool};
use crate::{
    errors::AmmError, exchanges::make_call_request, uniswap_v3::IErc20, AutomatedMarketMaker,
};
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
    struct TickData {
        bool initialized;
        int24 tick;
        int128 liquidityNet;
    }

    function tick_constructor(
        address pool,
        bool zeroForOne,
        int24 currentTick,
        uint16 numTicks,
        int24 tickSpacing
    ) returns (TickData[], uint64);
);

pub async fn get_v3_pool_data_batch_request<M: TracingProvider>(
    pool: &mut UniswapV3Pool,
    block: Option<u64>,
    provider: Arc<M>,
) -> Result<(), AmmError> {
    let to = pool.address;

    let (token_a, token_b, liquidity, fee, tick_spacing, slot0) = join!(
        make_call_request(token0Call::new(()), provider.clone(), to, block),
        make_call_request(token1Call::new(()), provider.clone(), to, block),
        make_call_request(liquidityCall::new(()), provider.clone(), to, block),
        make_call_request(feeCall::new(()), provider.clone(), to, block),
        make_call_request(tickSpacingCall::new(()), provider.clone(), to, block),
        make_call_request(slot0Call::new(()), provider.clone(), to, block)
    );

    pool.token_a = token_a?._0.into();
    pool.token_b = token_b?._0.into();
    pool.liquidity = liquidity?._0;
    pool.fee = fee?._0;
    pool.tick_spacing = tick_spacing?._0;

    let slot0 = slot0?;
    let (sqrt_price, tick) = (slot0._0, slot0._1);

    pool.sqrt_price = sqrt_price;
    pool.tick = tick;

    let (dec_a, dec_b) = join!(
        make_call_request(IErc20::decimalsCall::new(()), provider.clone(), pool.token_a, block),
        make_call_request(IErc20::decimalsCall::new(()), provider.clone(), pool.token_b, block)
    );

    pool.token_a_decimals = dec_a?._0;
    pool.token_b_decimals = dec_b?._0;

    let (r0, r1) = join!(
        make_call_request(
            IErc20::balanceOfCall::new((pool.address,)),
            provider.clone(),
            pool.token_a,
            block,
        ),
        make_call_request(
            IErc20::balanceOfCall::new((pool.address,)),
            provider,
            pool.token_b,
            block,
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

    let req =
        CallRequest { to: None, input: CallInput::new(bytecode.into()), ..Default::default() };

    let res = middleware
        .eth_call(req, block_number.map(Into::into), None, None)
        .await
        .unwrap();

    let return_data = tick_constructorCall::abi_decode_returns(&res, false).unwrap();

    Ok((return_data._0, return_data._1))
}
