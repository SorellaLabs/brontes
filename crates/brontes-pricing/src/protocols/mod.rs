pub mod errors;
pub mod factory;
pub mod lazy;
pub mod uniswap_v2;
pub mod uniswap_v3;
pub mod uniswap_v3_math;

use std::sync::Arc;

use alloy_primitives::{Address, Log, U256};
use alloy_rlp::{Decodable, Encodable};
use alloy_sol_types::SolCall;
use async_trait::async_trait;
use brontes_types::{normalized_actions::Actions, traits::TracingProvider};
use malachite::Rational;
use redefined::{self_convert_redefined, RedefinedConvert};
use reth_db::{
    table::{Compress, Decompress},
    DatabaseError,
};
use reth_primitives::BufMut;
use reth_rpc_types::{CallInput, CallRequest};
use serde::{Deserialize, Serialize};

use crate::protocols::errors::{AmmError, ArithmeticError, EventLogError, SwapSimulationError};

#[allow(non_camel_case_types)]
#[derive(
    Debug,
    PartialEq,
    Clone,
    Copy,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    rkyv::Serialize,
    rkyv::Deserialize,
    rkyv::Archive,
    strum::Display,
    strum::EnumString,
)]
pub enum StaticBindingsDb {
    UniswapV2,
    SushiSwapV2,
    UniswapV3,
    SushiSwapV3,
    CurveCryptoSwap,
    AaveV2,
    AaveV3,
    UniswapX,
    CurveV1BasePool,
    CurveV1MetaPool,
    CurveV2BasePool,
    CurveV2MetaPool,
    CurveV2PlainPool,
}

impl Encodable for StaticBindingsDb {
    fn encode(&self, out: &mut dyn BufMut) {
        match self {
            StaticBindingsDb::UniswapV2 => 0u64.encode(out),
            StaticBindingsDb::SushiSwapV2 => 1u64.encode(out),
            StaticBindingsDb::UniswapV3 => 2u64.encode(out),
            StaticBindingsDb::SushiSwapV3 => 3u64.encode(out),
            StaticBindingsDb::CurveCryptoSwap => 4u64.encode(out),
            StaticBindingsDb::AaveV2 => 5u64.encode(out),
            StaticBindingsDb::AaveV3 => 6u64.encode(out),
            StaticBindingsDb::UniswapX => 7u64.encode(out),
            StaticBindingsDb::CurveV1BasePool => 8u64.encode(out),
            StaticBindingsDb::CurveV1MetaPool => 9u64.encode(out),
            StaticBindingsDb::CurveV2BasePool => 10u64.encode(out),
            StaticBindingsDb::CurveV2MetaPool => 11u64.encode(out),
            StaticBindingsDb::CurveV2PlainPool => 12u64.encode(out),
        }
    }
}

impl Decodable for StaticBindingsDb {
    fn decode(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        let self_int = u64::decode(buf)?;

        let this = match self_int {
            0 => StaticBindingsDb::UniswapV2,
            1 => StaticBindingsDb::SushiSwapV2,
            2 => StaticBindingsDb::UniswapV3,
            3 => StaticBindingsDb::SushiSwapV3,
            4 => StaticBindingsDb::CurveCryptoSwap,
            5 => StaticBindingsDb::AaveV2,
            6 => StaticBindingsDb::AaveV3,
            7 => StaticBindingsDb::UniswapX,
            8 => StaticBindingsDb::CurveV1BasePool,
            9 => StaticBindingsDb::CurveV1MetaPool,
            10 => StaticBindingsDb::CurveV2BasePool,
            11 => StaticBindingsDb::CurveV2MetaPool,
            12 => StaticBindingsDb::CurveV2PlainPool,
            _ => unreachable!("no enum variant"),
        };

        Ok(this)
    }
}

impl Compress for StaticBindingsDb {
    type Compressed = Vec<u8>;

    fn compress_to_buf<B: reth_primitives::bytes::BufMut + AsMut<[u8]>>(self, buf: &mut B) {
        let mut encoded = Vec::new();
        self.encode(&mut encoded);
        buf.put_slice(&encoded);
    }
}

impl Decompress for StaticBindingsDb {
    fn decompress<B: AsRef<[u8]>>(value: B) -> Result<Self, reth_db::DatabaseError> {
        let binding = value.as_ref().to_vec();
        let buf = &mut binding.as_slice();
        StaticBindingsDb::decode(buf).map_err(|_| DatabaseError::Decode)
    }
}

self_convert_redefined!(StaticBindingsDb);

async fn make_call_request<C: SolCall, T: TracingProvider>(
    call: C,
    provider: Arc<T>,
    to: Address,
    block: Option<u64>,
) -> eyre::Result<C::Return> {
    let encoded = call.abi_encode();
    let req =
        CallRequest { to: Some(to), input: CallInput::new(encoded.into()), ..Default::default() };

    let res = provider
        .eth_call(req, block.map(Into::into), None, None)
        .await?;

    Ok(C::abi_decode_returns(&res, false)?)
}

#[async_trait]
pub trait AutomatedMarketMaker {
    fn address(&self) -> Address;
    // fn sync_on_event_signatures(&self) -> Vec<B256>;
    fn tokens(&self) -> Vec<Address>;
    fn calculate_price(&self, base_token: Address) -> Result<Rational, ArithmeticError>;
    fn sync_from_action(&mut self, action: Actions) -> Result<(), EventLogError>;
    fn sync_from_log(&mut self, log: Log) -> Result<(), EventLogError>;
    async fn populate_data<M: TracingProvider>(
        &mut self,
        block_number: Option<u64>,
        middleware: Arc<M>,
    ) -> Result<(), AmmError>;

    fn simulate_swap(
        &self,
        token_in: Address,
        amount_in: U256,
    ) -> Result<U256, SwapSimulationError>;
    fn simulate_swap_mut(
        &mut self,
        token_in: Address,
        amount_in: U256,
    ) -> Result<U256, SwapSimulationError>;
    fn get_token_out(&self, token_in: Address) -> Address;
}
