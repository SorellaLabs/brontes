use std::sync::Arc;

use alloy_rpc_types::{request::TransactionInput, TransactionRequest};
use alloy_sol_macro::sol;
use alloy_sol_types::SolCall;
use brontes_types::traits::TracingProvider;
use futures::TryFutureExt;

use super::UniswapV2Pool;
use crate::errors::AmmError;

sol!(
    IGetUniswapV2PoolDataBatchRequest,
    "./src/protocols/uniswap_v2/batch_request/GetUniswapV2PoolDataBatchRequestABI.json"
);

sol!(
    struct PoolData {
        address tokenA;
        uint8 tokenADecimals;
        address tokenB;
        uint8 tokenBDecimals;
        uint112 reserve0;
        uint112 reserve1;
    }

    function data_constructor(address[] memory pools) returns(PoolData[]);
);

fn populate_pool_data_from_tokens(mut pool: UniswapV2Pool, pool_data: PoolData) -> UniswapV2Pool {
    pool.token_a = pool_data.tokenA;
    pool.token_a_decimals = pool_data.tokenADecimals;
    pool.token_b = pool_data.tokenB;
    pool.token_b_decimals = pool_data.tokenBDecimals;
    pool.reserve_0 = pool_data.reserve0.to();
    pool.reserve_1 = pool_data.reserve1.to();

    pool
}

pub async fn get_v2_pool_data<M: TracingProvider>(
    pool: &mut UniswapV2Pool,
    block: Option<u64>,
    middleware: Arc<M>,
) -> Result<(), AmmError> {
    let mut bytecode = IGetUniswapV2PoolDataBatchRequest::BYTECODE.to_vec();
    data_constructorCall::new((vec![pool.address],)).abi_encode_raw(&mut bytecode);

    let req = TransactionRequest {
        to: None,
        input: TransactionInput::new(bytecode.into()),
        ..Default::default()
    };

    let res = middleware
        .eth_call_light(req, block.unwrap().into())
        .map_err(|e| eyre::eyre!("v2 state call failed, err={}", e))
        .await?;

    let mut return_data = data_constructorCall::abi_decode_returns(&res, false)?;
    *pool = populate_pool_data_from_tokens(pool.to_owned(), return_data._0.remove(0));
    Ok(())
}
