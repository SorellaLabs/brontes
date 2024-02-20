use std::{
    str::{from_utf8, FromStr},
    sync::Arc,
};

use alloy_primitives::{hex, Bytes, FixedBytes, U256};
use alloy_sol_macro::sol;
use alloy_sol_types::SolCall;
use brontes_types::traits::TracingProvider;
use reth_primitives::{Address, Bytecode, StorageValue};
use reth_rpc_types::{request::TransactionInput, TransactionRequest};

use super::UniswapV3Pool;
use crate::errors::AmmError;
sol!(
    IGetUniswapV3TickDataBatchRequest,
    "./src/protocols/uniswap_v3/batch_request/GetUniswapV3TickDataBatchRequestABI.json"
);
sol!(
    IGetERC20DataRequest,
    "./src/protocols/uniswap_v3/batch_request/GetERC20DataABI.json"
);

sol!(
    struct ERC20Data {
        uint256 balance;
        uint8 decimals;
    }
    function getERC20Data(address token0, address token1, address pool) returns (ERC20Data[]);
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

fn extract_uni_v3_immutables(bytecode: Bytes) -> (Address, Address, u32, i32) {
    // Position of the immutables in the bytecode
    let token0_range = 4542..4542 + 40;
    let token1_range = 9128..9128 + 40;
    let fee_range = 6682..6682 + 6;
    let tick_spacing_range = 6146..6146 + 64;

    // Slices
    let token0_slice = &bytecode[token0_range];
    let token1_slice = &bytecode[token1_range];
    let fee_slice = &bytecode[fee_range];
    let tick_spacing_slice = &bytecode[tick_spacing_range];

    // To UTF-8 String
    let token0 = from_utf8(token0_slice).unwrap();
    let token1 = from_utf8(token1_slice).unwrap();
    let fee = from_utf8(fee_slice).unwrap();
    let tick_spacing = from_utf8(tick_spacing_slice).unwrap();

    // Convert tokens to addresses
    let token0 = Address::from_str(token0).unwrap();
    let token1 = Address::from_str(token1).unwrap();

    // Convert fee to uint
    let fee = u32::from_str_radix(fee, 16).unwrap();
    // Convert tick_spacing to int
    let tick_spacing = i32::from_str_radix(tick_spacing, 16).unwrap();

    (token0, token1, fee, tick_spacing)
}

pub async fn get_v3_pool_data_batch_request<M: TracingProvider>(
    pool: &mut UniswapV3Pool,
    block_number: Option<u64>,
    middleware: Arc<M>,
) -> Result<(), AmmError> {
    // Pool Storage Slots
    let slot0_slot: FixedBytes<32> = FixedBytes::new([0u8; 32]);
    let liquidity_slot: FixedBytes<32> = FixedBytes::with_last_byte(4);

    // Fetch from db
    let slot0: Option<StorageValue> = middleware
        .get_storage(block_number, pool.address, slot0_slot)
        .await?;
    let liquidity: Option<StorageValue> = middleware
        .get_storage(block_number, pool.address, liquidity_slot)
        .await?;

    // Fetch bytecode
    let pool_bytecode: Option<Bytecode> =
        middleware.get_bytecode(block_number, pool.address).await?;

    // Decode slot0 into sqrt_price and tick
    if let Some(slot0) = slot0 {
        let slot0 = hex::encode::<[u8; 32]>(slot0.to_be_bytes());
        let sqrt_price = U256::from_str_radix(&slot0[slot0.len() - 40..], 16).unwrap();
        let tick = i32::from_str_radix(&slot0[slot0.len() - 46..][..6], 16).unwrap();
        pool.sqrt_price = sqrt_price;
        pool.tick = tick;
    }

    // Decode liquidity
    if let Some(liquidity) = liquidity {
        let liquidity = hex::encode::<[u8; 32]>(liquidity.to_be_bytes());
        let liquidity = u128::from_str_radix(&liquidity[liquidity.len() - 16..], 16).unwrap();
        pool.liquidity = liquidity;
    }

    // Extract token0, token1, fee, tick_spacing from bytecode
    if let Some(pool_bytecode) = pool_bytecode {
        let pool_bytecode = Bytes::from(hex::encode_prefixed(pool_bytecode.bytecode.as_ref()));
        let (token0, token1, fee, tick_spacing) = extract_uni_v3_immutables(pool_bytecode);
        pool.fee = fee;
        pool.tick_spacing = tick_spacing;
        pool.token_a = token0;
        pool.token_b = token1;
    }

    let mut bytecode = IGetERC20DataRequest::BYTECODE.to_vec();
    getERC20DataCall::new((pool.token_a, pool.token_b, pool.address)).abi_encode_raw(&mut bytecode);
    let req = TransactionRequest {
        to: None,
        input: TransactionInput::new(bytecode.into()),
        ..Default::default()
    };
    let res = middleware
        .eth_call(req, block_number.map(|i| i.into()), None, None)
        .await
        .map_err(|_| eyre::eyre!("v3 data fetch call failed"))?;

    let return_data = getERC20DataCall::abi_decode_returns(&res, false)?;

    pool.reserve_0 = return_data._0[0].balance;
    pool.reserve_1 = return_data._0[1].balance;
    pool.token_a_decimals = return_data._0[0].decimals;
    pool.token_b_decimals = return_data._0[1].decimals;

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
