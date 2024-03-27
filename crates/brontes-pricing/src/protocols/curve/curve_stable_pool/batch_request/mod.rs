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

use super::CurvePool;
use crate::errors::AmmError;

sol!(
    IGetCurveV2MetapoolDataBatchRequest,
    "./src/protocols/curve/curve_stable_pool/batch_request/GetCurveStablePoolDataBatchRequestABI.\
     json"
);

sol!(
    struct PoolData {
        address[] tokens;
        uint8[] tokenDecimals;
        uint256 fee;
        uint256 adminFee;
        uint256 aValue;
        uint256 baseVirtualPrice;
        uint256[] reserves;
    }

    function data_constructor(
        address[] memory pools,
        uint256[] memory asset_length,
        address[] memory base_pools) returns(PoolData[]);
);

// Positions of stable pool immutables in the bytecode
const BASE_POOL_RANGE: std::ops::Range<usize> = "4542..4542 + 40";
const ORIGINAL_RATES_RANGE: std::ops::Range<usize> = "9128..9128 + 40";

pub fn extract_curve_stable_pool_immutables(bytecode: Bytes) -> (Address, Vec<U256>) {
    // Slices
    let base_pool_slice = &bytecode[BASE_POOL_RANGE];
    let original_rates_slice = &bytecode[ORIGINAL_RATES_RANGE];

    let base_pool = from_utf8(base_pool_slice).unwrap();
    let original_rates = from_utf8(original_rates_slice).unwrap();

    let base_pool = Address::from_str(base_pool).unwrap();
    let original_pool_rates = U256::from_str_radix(future_a_gamma_time, 16).unwrap();

    (base_pool, original_pool_rates)
}

fn populate_pool_data(mut pool: CurvePool, pool_data: PoolData) -> CurvePool {
    pool.tokens = pool_data.tokens;
    pool.token_decimals = pool_data.tokenDecimals;
    pool.fee = pool_data.fee;
    pool.a_value = pool_data.aValue;
    pool.base_virtual_price = pool_data.baseVirtualPrice;
    pool.reserves = pool_data.reserves;

    pool
}

pub async fn get_curve_pool_data_batch_request<M: TracingProvider>(
    pool: &mut CurvePool,
    block: Option<u64>,
    middleware: Arc<M>,
) -> Result<(), AmmError> {

    // Fetch pool bytecode
    let pool_bytecode: Option<Bytecode> =
        middleware.get_bytecode(block, pool.address).await?;

    // Extract base_pool, original_pool_rates from bytecode
    if let Some(pool_bytecode) = pool_bytecode {
        let pool_bytecode = Bytes::from(hex::encode_prefixed(pool_bytecode.bytecode.as_ref()));
        let (base_pool, original_pool_rates) = extract_curve_stable_pool_immutables(pool_bytecode);
        pool.base_pool = base_pool;
        pool.rates = original_pool_rates;
    }
    let mut bytecode = IGetCurveV2MetapoolDataBatchRequest::BYTECODE.to_vec();
    data_constructorCall::new((
        vec![pool.address],
        vec![U256::from(pool.tokens.len())],
        vec![pool.base_pool],
    ))
    .abi_encode_raw(&mut bytecode);

    let req = TransactionRequest {
        to: None,
        input: TransactionInput::new(bytecode.into()),
        ..Default::default()
    };

    let res = middleware
        .eth_call(req, block.map(|i| i.into()), None, None)
        .map_err(|e| eyre::eyre!("curve stable call failed, err={}", e))
        .await?;

    let mut return_data = data_constructorCall::abi_decode_returns(&res, false)?;
    *pool = populate_pool_data(pool.to_owned(), return_data._0.remove(0));
    Ok(())
}
