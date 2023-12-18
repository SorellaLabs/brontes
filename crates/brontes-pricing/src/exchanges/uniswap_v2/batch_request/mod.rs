use std::sync::Arc;

use alloy_sol_macro::sol;
use alloy_sol_types::SolCall;
use brontes_types::traits::TracingProvider;
use reth_rpc_types::{CallInput, CallRequest};

use super::UniswapV2Pool;
use crate::{errors::AmmError, AutomatedMarketMaker};

sol!(
    IGetUniswapV2PoolDataBatchRequest,
    "./src/exchanges/uniswap_v2/batch_request/GetUniswapV2PoolDataBatchRequestABI.json"
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

// pub async fn get_pairs_batch_request<M: TracingProvider>(
//     factory: H160,
//     from: U256,
//     step: U256,
//     middleware: Arc<M>,
// ) -> Result<Vec<H160>, AmmError> {
//     tracing::info!("getting pairs {}-{}", from, step);
//
//     let mut pairs = vec![];
//
//     let constructor_args =
//         Token::Tuple(vec![Token::Uint(from), Token::Uint(step),
// Token::Address(factory)]);
//
//     let deployer = IGetUniswapV2PairsBatchRequest::deploy(middleware,
// constructor_args)?;     let return_data: Bytes = deployer.call_raw().await?;
//
//     let return_data_tokens =
//         ethers::abi::decode(&
// [ParamType::Array(Box::new(ParamType::Address))], &return_data)?;
//
//     for token_array in return_data_tokens {
//         if let Some(arr) = token_array.into_array() {
//             for token in arr {
//                 if let Some(addr) = token.into_address() {
//                     if !addr.is_zero() {
//                         pairs.push(addr);
//                     }
//                 }
//             }
//         }
//     }
//
//     Ok(pairs)
// }
//

fn populate_pool_data_from_tokens(mut pool: UniswapV2Pool, pool_data: PoolData) -> UniswapV2Pool {
    pool.token_a = pool_data.tokenA;
    pool.token_a_decimals = pool_data.tokenADecimals;
    pool.token_b = pool_data.tokenB;
    pool.token_b_decimals = pool_data.tokenBDecimals;
    pool.reserve_0 = pool_data.reserve0;
    pool.reserve_1 = pool_data.reserve1;

    pool
}

pub async fn get_v2_pool_data<M: TracingProvider>(
    pool: &mut UniswapV2Pool,
    block: Option<u64>,
    middleware: Arc<M>,
) -> Result<(), AmmError> {
    let mut bytecode = IGetUniswapV2PoolDataBatchRequest::BYTECODE.to_vec();
    data_constructorCall::new((vec![pool.address],)).abi_encode_raw(&mut bytecode);

    let req =
        CallRequest { to: None, input: CallInput::new(bytecode.into()), ..Default::default() };

    let res = middleware
        .eth_call(req, block.map(|i| i.into()), None, None)
        .await
        .unwrap();

    let mut return_data = data_constructorCall::abi_decode_returns(&*res, false).unwrap();
    *pool = populate_pool_data_from_tokens(pool.to_owned(), return_data._0.remove(0));
    Ok(())
}

pub async fn get_amm_data_batch_request<M: TracingProvider>(
    amms: &mut [UniswapV2Pool],
    middleware: Arc<M>,
) -> Result<(), AmmError> {
    // tracing::info!("getting data for {} AMMs", amms.len());
    //
    // let mut target_addresses = vec![];
    // for amm in amms.iter() {
    //     target_addresses.push(Token::Address(amm.address()));
    // }
    //
    // let constructor_args = Token::Tuple(vec![Token::Array(target_addresses)]);
    //
    // let deployer = IGetUniswapV2PoolDataBatchRequest::deploy(middleware.clone(),
    // constructor_args)?;
    //
    // let return_data: Bytes = deployer.call_raw().await?;
    // let return_data_tokens = ethers::abi::decode(
    //     &[ParamType::Array(Box::new(ParamType::Tuple(vec![
    //         ParamType::Address,   // token a
    //         ParamType::Uint(8),   // token a decimals
    //         ParamType::Address,   // token b
    //         ParamType::Uint(8),   // token b decimals
    //         ParamType::Uint(112), // reserve 0
    //         ParamType::Uint(112), // reserve 1
    //     ])))],
    //     &return_data,
    // )?;
    //
    // let mut pool_idx = 0;
    //
    // for tokens in return_data_tokens {
    //     if let Some(tokens_arr) = tokens.into_array() {
    //         for tup in tokens_arr {
    //             if let Some(pool_data) = tup.into_tuple() {
    //                 //If the pool token A is not zero, signaling that the pool
    // data was populated                 if let Some(address) =
    // pool_data[0].to_owned().into_address() {                     if
    // !address.is_zero() {                         //Update the pool data
    //                         if let AMM::UniswapV2Pool(uniswap_v2_pool) = amms
    //                             .get_mut(pool_idx)
    //                             .expect("Pool idx should be in bounds")
    //                         {
    //                             if let Some(pool) =
    // populate_pool_data_from_tokens(
    // uniswap_v2_pool.to_owned(),                                 pool_data,
    //                             ) {
    //                                 tracing::trace!(?pool);
    //                                 *uniswap_v2_pool = pool;
    //                             }
    //                         }
    //                     }
    //                 }
    //
    //                 pool_idx += 1;
    //             }
    //         }
    //     }
    // }
    todo!();

    Ok(())
}

pub async fn get_v2_pool_data_batch_request<M: TracingProvider>(
    pool: &mut UniswapV2Pool,
    middleware: Arc<M>,
) -> Result<(), AmmError> {
    // tracing::info!(?pool.address, "getting pool data");
    // let constructor_args =
    // Token::Tuple(vec![Token::Array(vec![Token::Address(pool.address)])]);
    //
    // let deployer = IGetUniswapV2PoolDataBatchRequest::deploy(middleware.clone(),
    // constructor_args)?;
    //
    // let return_data: Bytes = deployer.call_raw().await?;
    // let return_data_tokens = ethers::abi::decode(
    //     &[ParamType::Array(Box::new(ParamType::Tuple(vec![
    //         ParamType::Address,   // token a
    //         ParamType::Uint(8),   // token a decimals
    //         ParamType::Address,   // token b
    //         ParamType::Uint(8),   // token b decimals
    //         ParamType::Uint(112), // reserve 0
    //         ParamType::Uint(112), // reserve 1
    //     ])))],
    //     &return_data,
    // )?;
    //
    // for tokens in return_data_tokens {
    //     if let Some(tokens_arr) = tokens.into_array() {
    //         for tup in tokens_arr {
    //             let pool_data = tup
    //                 .into_tuple()
    //                 .ok_or(AMMError::BatchRequestError(pool.address))?;
    //
    //             *pool = populate_pool_data_from_tokens(pool.to_owned(),
    // pool_data)
    // .ok_or(AMMError::BatchRequestError(pool.address))?;         }
    //     }
    // }
    todo!();

    Ok(())
}
