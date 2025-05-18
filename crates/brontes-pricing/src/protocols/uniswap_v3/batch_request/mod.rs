use std::{
    str::{from_utf8, FromStr},
    sync::Arc,
};

use alloy_primitives::{hex, Bytes, FixedBytes, U256};
use alloy_sol_macro::sol;
use alloy_sol_types::SolCall;
use brontes_types::traits::TracingProvider;
use futures::TryFutureExt;
use reth_primitives::{Address, Bytecode, StorageValue};
use reth_rpc_types::{request::TransactionInput, TransactionRequest};

mod test_bytecodes;
use super::UniswapV3Pool;
use crate::errors::AmmError;
sol!(
    IGetUniswapV3TickDataBatchRequest,
    "./src/protocols/uniswap_v3/batch_request/GetUniswapV3TickDataBatchRequest.json"
);
sol!(IGetERC20DataRequest, "./src/protocols/uniswap_v3/batch_request/GetERC20DataABI.json");

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
// Positions of Uni v3 immutables in the bytecode
const TOKEN0_RANGE: std::ops::Range<usize> = 4542..4542 + 40;
const TOKEN1_RANGE: std::ops::Range<usize> = 9128..9128 + 40;
const FEE_RANGE: std::ops::Range<usize> = 6682..6682 + 6;
const TICK_SPACING_RANGE: std::ops::Range<usize> = 6146..6146 + 64;

//TODO: Good first issue for someone to prune the unnecessary data we are
// loading for the pools TODO: We don't need ticks or fees, we should already
// have token 0 & token 1 TODO: We also don't need bytecode or tick spacing
pub fn extract_uni_v3_immutables(bytecode: Bytes) -> eyre::Result<(Address, Address, u32, i32)> {
    // Slices
    let token0_slice = &bytecode[TOKEN0_RANGE];
    let token1_slice = &bytecode[TOKEN1_RANGE];
    let fee_slice = &bytecode[FEE_RANGE];
    let tick_spacing_slice = &bytecode[TICK_SPACING_RANGE];

    // To UTF-8 String
    let token0 = from_utf8(token0_slice)?;
    let token1 = from_utf8(token1_slice)?;
    let fee = from_utf8(fee_slice)?;
    let tick_spacing = from_utf8(tick_spacing_slice)?;

    // Convert tokens to addresses
    let token0 = Address::from_str(token0)?;
    let token1 = Address::from_str(token1)?;

    // Convert fee to uint
    let fee = u32::from_str_radix(fee, 16)?;
    // Convert tick_spacing to int
    let tick_spacing = i32::from_str_radix(tick_spacing, 16)?;

    Ok((token0, token1, fee, tick_spacing))
}

pub async fn get_v3_pool_data_batch_request<M: TracingProvider>(
    pool: &mut UniswapV3Pool,
    block_number: Option<u64>,
    middleware: Arc<M>,
) -> Result<(), AmmError> {
    let call_data = data_constructorCall::new((vec![pool.address],)).abi_encode();

    let req = TransactionRequest {
        to: Some(Address::from_str("0x23e5b07d8e216340Cf34252c81a0D19BE13FB22f").unwrap()),
        input: TransactionInput::new(call_data.into()),
        ..Default::default()
    };

    let res = middleware
        .eth_call_light(req, block_number.unwrap().into())
        .map_err(|e| eyre::eyre!("v3 state call failed, err={}", e))
        .await?;

    let return_data = data_constructorCall::abi_decode_returns(&res, false)?;
    pool.sqrt_price = return_data._0[0].sqrtPrice;
    pool.tick = return_data._0[0].tick;
    pool.liquidity = return_data._0[0].liquidity;
    pool.token_a = return_data._0[0].tokenA;
    pool.token_b = return_data._0[0].tokenB;
    pool.fee = return_data._0[0].fee;
    pool.tick_spacing = return_data._0[0].tickSpacing;

    let mut bytecode = IGetERC20DataRequest::BYTECODE.to_vec();
    getERC20DataCall::new((pool.token_a, pool.token_b, pool.address)).abi_encode_raw(&mut bytecode);
    let req = TransactionRequest {
        to: None,
        input: TransactionInput::new(bytecode.into()),
        ..Default::default()
    };
    let res = middleware
        .eth_call_light(req, block_number.unwrap().into())
        .await
        .map_err(|e| eyre::eyre!("v3 data fetch call failed, err={}", e))?;

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
    let call_data = tick_constructorCall::new((
        pool.address,
        zero_for_one,
        tick_start,
        num_ticks,
        pool.tick_spacing,
    ))
    .abi_encode();

    let req = TransactionRequest {
        to: Some(Address::from_str("0x23e5b07d8e216340Cf34252c81a0D19BE13FB22f").unwrap()),
        input: TransactionInput::new(call_data.into()),
        ..Default::default()
    };

    let res = middleware
        .eth_call(req, block_number.map(Into::into), None, None)
        .await
        .unwrap();

    let return_data = tick_constructorCall::abi_decode_returns(&res, false).unwrap();

    Ok((return_data._0, return_data._1))
}

#[cfg(test)]
mod tests {
    use test_bytecodes::{V2_DAI_MKR, V3_USDC_ETH, V3_WBTC_ETH};

    use super::*;

    #[test]
    fn test_v3_bytecodes() {
        let (token0, token1, fees, tick_spacing) =
            extract_uni_v3_immutables(V3_WBTC_ETH.into()).unwrap();

        assert_eq!(
            token0,
            Address::from_str("0x2260FAC5E5542a773Aa44fBCfeDf7C193bc2C599").unwrap()
        );
        assert_eq!(
            token1,
            Address::from_str("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2").unwrap()
        );
        assert_eq!(fees, 3000);
        assert_eq!(tick_spacing, 60);

        let (token0, token1, fees, tick_spacing) =
            extract_uni_v3_immutables(V3_USDC_ETH.into()).unwrap();

        assert_eq!(
            token0,
            Address::from_str("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48").unwrap()
        );

        assert_eq!(
            token1,
            Address::from_str("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2").unwrap()
        );

        assert_eq!(fees, 500);

        assert_eq!(tick_spacing, 10);
    }

    // Test fails with error ParseIntError { kind: PosOverflow }
    #[test]
    #[should_panic]
    fn test_fail_v2_bytecode() {
        let (token0, token1, _fees, _tick_spacing) =
            extract_uni_v3_immutables(V2_DAI_MKR.into()).unwrap();

        assert_eq!(
            token0,
            Address::from_str("0x6B175474E89094C44Da98b954EedeAC495271d0F ").unwrap()
        );

        assert_eq!(
            token1,
            Address::from_str("0x9f8F72aA9304c8B593d555F12eF6589cC3A579A2").unwrap()
        );
    }

    #[brontes_macros::test]
    #[cfg(feature = "local-reth")]
    async fn test_v3_slot0() {
        let loader = brontes_core::test_utils::TraceLoader::new().await;
        let provider = loader.get_provider();

        let block_number: u64 = 19450752;
        let pool_address = Address::from_str("0xcbcdf9626bc03e24f779434178a73a0b4bad62ed").unwrap();
        let slot0_slot: FixedBytes<32> = FixedBytes::new([0u8; 32]);

        let storage_value = provider
            .get_storage(Some(block_number), pool_address, slot0_slot)
            .await
            .unwrap();

        if let Some(value) = storage_value {
            let slot0 = hex::encode::<[u8; 32]>(value.to_be_bytes());
            let sqrt_price = U256::from_str_radix(&slot0[slot0.len() - 40..], 16).unwrap();
            let tick = i32::from_str_radix(&slot0[slot0.len() - 46..][..6], 16).unwrap();

            // Ref: https://evm.storage/eth/19450752/0xcbcdf9626bc03e24f779434178a73a0b4bad62ed/slot0#map
            assert_eq!(sqrt_price, U256::from_str("34181474658983484482097063224900296").unwrap());

            assert_eq!(tick, i32::from_str("259510").unwrap());
        };
    }

    #[brontes_macros::test]
    #[cfg(feature = "local-reth")]
    async fn test_v3_liquidity() {
        let loader = brontes_core::test_utils::TraceLoader::new().await;
        let provider = loader.get_provider();
        let block_number: u64 = 19450752;
        let pool_address = Address::new(hex!("cbcdf9626bc03e24f779434178a73a0b4bad62ed"));

        let liquidity_slot: FixedBytes<32> = FixedBytes::with_last_byte(4);
        let storage_value = provider
            .get_storage(Some(block_number), pool_address, liquidity_slot)
            .await
            .unwrap();

        if let Some(value) = storage_value {
            let liquidity = hex::encode::<[u8; 32]>(value.to_be_bytes());
            let liquidity = u128::from_str_radix(&liquidity[liquidity.len() - 16..], 16).unwrap();

            // Ref: https://evm.storage/eth/19450752/0xcbcdf9626bc03e24f779434178a73a0b4bad62ed/liquidity#map
            assert_eq!(liquidity, u128::from_str("1266853986742771321").unwrap());
        };
    }
}
