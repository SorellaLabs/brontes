pub mod errors;
pub mod factory;
pub mod lazy;
pub mod uniswap_v2;
pub mod uniswap_v3;
pub mod uniswap_v3_math;

use std::sync::Arc;

use alloy_primitives::{Address, Log, U256};
use alloy_rlp::{Decodable, Encodable, RlpDecodable, RlpEncodable};
use alloy_sol_types::SolCall;
use async_trait::async_trait;
use brontes_types::{extra_processing::Pair, normalized_actions::Actions, traits::TracingProvider};
use malachite::Rational;
use redefined::{self_convert_redefined, RedefinedConvert};
use reth_codecs::{impl_compact_for_bytes, Compact};
use reth_db::{
    table::{Compress, Decompress},
    DatabaseError,
};
use reth_primitives::BufMut;
use reth_rpc_types::{CallInput, CallRequest};
use rkyv::Deserialize as rkyv_Deserialize;
use serde::{Deserialize, Serialize};
use sorella_db_databases::clickhouse::{self, DbRow};
use tracing::error;

use crate::{
    lazy::{PoolFetchError, PoolFetchSuccess},
    protocols::errors::{AmmError, ArithmeticError, EventLogError, SwapSimulationError},
    uniswap_v2::UniswapV2Pool,
    uniswap_v3::UniswapV3Pool,
    LoadResult, PoolState,
};

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
    brontes_macros::ToConstByte,
)]
#[repr(u8)]
pub enum Protocol {
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

impl Encodable for Protocol {
    fn encode(&self, out: &mut dyn BufMut) {
        let encoded = rkyv::to_bytes::<_, 256>(self).unwrap();
        out.put_slice(&encoded)
    }
}

impl Decodable for Protocol {
    fn decode(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        println!("{:#?}", buf);
        let archived: &ArchivedProtocol = unsafe { rkyv::archived_root::<Self>(buf) };
        println!("{:?}", archived);

        let this = ArchivedProtocol::deserialize(&archived, &mut rkyv::Infallible).unwrap();
        Ok(this)
    }
}

impl Compress for Protocol {
    type Compressed = Vec<u8>;

    fn compress_to_buf<B: reth_primitives::bytes::BufMut + AsMut<[u8]>>(self, buf: &mut B) {
        let mut encoded = Vec::new();
        self.encode(&mut encoded);
        buf.put_slice(&encoded);
    }
}

impl Decompress for Protocol {
    fn decompress<B: AsRef<[u8]>>(value: B) -> Result<Self, reth_db::DatabaseError> {
        let binding = value.as_ref().to_vec();
        let buf = &mut binding.as_slice();
        Protocol::decode(buf).map_err(|_| DatabaseError::Decode)
    }
}

fn test() {
    let c = Protocol::UniswapV2.to_byte();
}

impl Protocol {
    pub(crate) async fn try_load_state<T: TracingProvider>(
        self,
        address: Address,
        provider: Arc<T>,
        block_number: u64,
        pool_pair: Pair,
    ) -> Result<PoolFetchSuccess, PoolFetchError> {
        match self {
            Self::UniswapV2 | Self::SushiSwapV2 => {
                let (pool, res) = if let Ok(pool) =
                    UniswapV2Pool::new_load_on_block(address, provider.clone(), block_number - 1)
                        .await
                {
                    (pool, LoadResult::Ok)
                } else {
                    (
                        UniswapV2Pool::new_load_on_block(address, provider, block_number)
                            .await
                            .map_err(|e| {
                                (address, Protocol::UniswapV2, block_number, pool_pair, e)
                            })?,
                        LoadResult::PoolInitOnBlock,
                    )
                };

                Ok((
                    block_number,
                    address,
                    PoolState::new(crate::types::PoolVariants::UniswapV2(pool)),
                    res,
                ))
            }
            Self::UniswapV3 | Self::SushiSwapV3 => {
                let (pool, res) = if let Ok(pool) =
                    UniswapV3Pool::new_from_address(address, block_number - 1, provider.clone())
                        .await
                {
                    (pool, LoadResult::Ok)
                } else {
                    (
                        UniswapV3Pool::new_from_address(address, block_number, provider)
                            .await
                            .map_err(|e| {
                                (address, Protocol::UniswapV3, block_number, pool_pair, e)
                            })?,
                        LoadResult::PoolInitOnBlock,
                    )
                };

                Ok((
                    block_number,
                    address,
                    PoolState::new(crate::types::PoolVariants::UniswapV3(pool)),
                    res,
                ))
            }
            rest => {
                error!(protocol=?rest, "no state updater is build for");
                Err((address, self, block_number, pool_pair, AmmError::UnsupportedProtocol))
            }
        }
    }
}

self_convert_redefined!(Protocol);

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
