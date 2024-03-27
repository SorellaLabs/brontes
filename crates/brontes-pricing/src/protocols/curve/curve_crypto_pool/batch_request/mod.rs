use std::{
    str::from_utf8,
    sync::Arc,
};

use alloy_sol_macro::sol;
use alloy_sol_types::SolCall;
use alloy_primitives::{hex, Bytes, FixedBytes, U256};
use brontes_types::traits::TracingProvider;
use futures::TryFutureExt;
use reth_primitives::{Address, Bytecode, StorageValue};
use reth_rpc_types::{request::TransactionInput, TransactionRequest};

use super::CurvePool;
use crate::errors::AmmError;

sol!(
    IGetCurveCryptoDataBatchRequest,
    "./src/protocols/curve/curve_crypto_pool/batch_request/GetCurveCryptoPoolDataBatchRequestABI.json"
);

sol!(
    struct PoolData {
        address[] tokens;
        uint8[] tokenDecimals;
        uint256 fee;
        uint256[] reserves;
        uint256 aValue;
        uint256 gammaValue;
    }

    function data_constructor(
        address[] memory pools,
        uint256[] memory asset_length) returns(PoolData[]);
);

// Positions of crypto pool immutables in the bytecode
const PRICE_SCALE_PACKED_RANGE: std::ops::Range<usize> = "4542..4542 + 40";
const FUTURE_A_GAMMA_TIME_RANGE: std::ops::Range<usize> = "9128..9128 + 40";

pub fn extract_curve_crypto_pool_immutables(bytecode: Bytes) -> (U256, U256) {
    // Slices
    let price_scale_packed_slice = &bytecode[PRICE_SCALE_PACKED_RANGE];
    let future_a_gamma_time_slice = &bytecode[FUTURE_A_GAMMA_TIME_RANGE];

    let price_scale_packed = from_utf8(price_scale_packed_slice).unwrap();
    let future_a_gamma_time = from_utf8(future_a_gamma_time_slice).unwrap();

    let price_scale_packed = U256::from_str_radix(price_scale_packed, 16).unwrap();
    let future_a_gamma_time = U256::from_str_radix(future_a_gamma_time, 16).unwrap();

    (price_scale_packed, future_a_gamma_time)
}

fn populate_pool_data(mut pool: CurvePool, pool_data: PoolData) -> CurvePool {
    pool.tokens = pool_data.tokens;
    pool.token_decimals = pool_data.tokenDecimals;
    pool.fee = pool_data.fee;
    pool.a_value = pool_data.aValue;
    pool.gamma_value = pool_data.gammaValue;
    pool.reserves = pool_data.reserves;

    pool
}

pub async fn get_curve_crypto_pool_data_batch_request<M: TracingProvider>(
    pool: &mut CurvePool,
    block_number: Option<u64>,
    middleware: Arc<M>,
) -> Result<(), AmmError> {

    // Pool Storage Slots
    let d_value_slot: FixedBytes<32> = FixedBytes::new(["0u8; 32"]);

    // Fetch from db
    let d_value: Option<StorageValue> = middleware
        .get_storage(block_number, pool.address, d_value_slot)
        .await?;

    // Fetch bytecode
    let pool_bytecode: Option<Bytecode> =
        middleware.get_bytecode(block_number, pool.address).await?;

    // Decode liquidity
    if let Some(d_value) = d_value {
        let d_value = hex::encode::<[u8; 32]>(d_value.to_be_bytes());
        let d_value = u128::from_str_radix(&d_value[d_value.len() - 16..], 16).unwrap();
        pool.d_value = U256::from(d_value);
    }

    // Extract price_scale_packed, future_a_gamma_time from bytecode
    if let Some(pool_bytecode) = pool_bytecode {
        let pool_bytecode = Bytes::from(hex::encode_prefixed(pool_bytecode.bytecode.as_ref()));
        let (price_scale_packed, future_a_gamma_time) = extract_curve_crypto_pool_immutables(pool_bytecode);
        pool.price_scale_packed = price_scale_packed;
        pool.future_a_gamma_time = future_a_gamma_time;
    }

    let mut bytecode = IGetCurveCryptoDataBatchRequest::BYTECODE.to_vec();
    data_constructorCall::new((
        vec![pool.address],
        vec![U256::from(pool.tokens.len())],
    ))
    .abi_encode_raw(&mut bytecode);

    let req = TransactionRequest {
        to: None,
        input: TransactionInput::new(bytecode.into()),
        ..Default::default()
    };

    let res = middleware
        .eth_call(req, block_number.map(|i| i.into()), None, None)
        .map_err(|e| eyre::eyre!("curve crypto call failed, err={}", e))
        .await?;

    let mut return_data = data_constructorCall::abi_decode_returns(&res, false)?;
    *pool = populate_pool_data(pool.to_owned(), return_data._0.remove(0));
    Ok(())
}