use std::{
    str::{from_utf8, FromStr},
    sync::Arc,
};

use alloy_primitives::Bytes;
use alloy_sol_macro::sol;
use alloy_sol_types::SolCall;
use brontes_types::traits::TracingProvider;
use futures::TryFutureExt;
use reth_primitives::Address;
use reth_rpc_types::{request::TransactionInput, TransactionRequest};

use crate::errors::AmmError;

sol!(
    IGetERC20DataBatchRequest,
    "./src/protocols/erc20/batch_request/GetERC20DataBatchRequest.json"
);

sol!(
    struct TokenData {
        address token;
        string name;
        string symbol;
        uint8 decimals;
    }

    function data_constructor(address[] memory tokens) returns (TokenData[]);
);

pub async fn get_erc20_data<M: TracingProvider>(
    tokens: Vec<Address>,
    block_number: Option<u64>,
    middleware: Arc<M>,
) -> Result<Vec<TokenData>, AmmError> {
    let mut bytecode = IGetERC20DataBatchRequest::BYTECODE.to_vec();
    data_constructorCall::new((tokens,)).abi_encode_raw(&mut bytecode);

    let req = TransactionRequest {
        to: None,
        input: TransactionInput::new(bytecode.into()),
        ..Default::default()
    };

    let res = middleware
        .eth_call_light(req, block_number.unwrap().into())
        .map_err(|e| eyre::eyre!("v3 state call failed, err={}", e))
        .await?;

    let return_data = data_constructorCall::abi_decode_returns(&res, false)?;
    Ok(return_data._0)
}
