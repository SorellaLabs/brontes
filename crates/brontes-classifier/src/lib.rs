use std::{fmt::Debug, sync::Arc};

use alloy_primitives::{Address, Bytes, Log};
use brontes_pricing::types::{DiscoveredPool, PoolUpdate};
use brontes_types::traits::TracingProvider;
use futures::Future;
use reth_db::mdbx::RO;

pub mod tree_builder;
pub use tree_builder::Classifier;
pub mod bindings;
use bindings::*;

#[cfg(feature = "tests")]
pub mod test_utils;

mod classifiers;
use alloy_sol_types::{sol, SolInterface};
use brontes_types::normalized_actions::Actions;
pub use classifiers::*;

// Actions
sol!(UniswapV2, "./classifier-abis/UniswapV2.json");
sol!(SushiSwapV2, "./classifier-abis/SushiSwapV2.json");
sol!(UniswapV3, "./classifier-abis/UniswapV3.json");
sol!(SushiSwapV3, "./classifier-abis/SushiSwapV3.json");
sol!(CurveCryptoSwap, "./classifier-abis/CurveCryptoSwap.json");
sol!(AaveV2, "./classifier-abis/AaveV2Pool.json");
sol!(AaveV3, "./classifier-abis/AaveV3Pool.json");
sol!(UniswapX, "./classifier-abis/UniswapXExclusiveDutchOrderReactor.json");

// Discovery
sol!(UniswapV2Factory, "./classifier-abis/UniswapV2Factory.json");
sol!(UniswapV3Factory, "./classifier-abis/UniswapV3Factory.json");
sol!(CurveV1MetapoolFactory, "./classifier-abis/CurveMetapoolFactoryV1.json");
sol!(CurveV2MetapoolFactory, "./classifier-abis/CurveMetapoolFactoryV2.json");
sol!(CurvecrvUSDFactory, "./classifier-abis/CurveCRVUSDFactory.json");
sol!(CurveCryptoSwapFactory, "./classifier-abis/CurveCryptoSwapFactory.json");
sol!(CurveTriCryptoFactory, "./classifier-abis/CurveTriCryptoFactory.json");
sol! {
    event Transfer(address indexed from, address indexed to, uint256 value);
    function name() public view returns (string);
    function symbol() public view returns (string);
    function decimals() public view returns (uint8);
    function totalSupply() public view returns (uint256);
}

pub trait ActionCollection: Sync + Send {
    fn dispatch(
        &self,
        sig: &[u8],
        trace_index: u64,
        data: StaticReturnBindings,
        return_data: Bytes,
        from_address: Address,
        target_address: Address,
        msg_sender: Address,
        logs: &Vec<Log>,
        db_tx: &brontes_database::libmdbx::tx::CompressedLibmdbxTx<RO>,
        block: u64,
        tx_idx: u64,
    ) -> Option<(PoolUpdate, Actions)>;
}

/// implements the above trait for decoding on the different binding enums
#[macro_export]
macro_rules! impl_decode_sol {
    ($enum_name:ident, $inner_type:path) => {
        impl TryDecodeSol for $enum_name {
            type DecodingType = $inner_type;

            fn try_decode(call_data: &[u8]) -> Result<Self::DecodingType, alloy_sol_types::Error> {
                Self::DecodingType::abi_decode(call_data, false)
            }
        }
    };
}

pub trait IntoAction: Debug + Send + Sync {
    fn get_signature(&self) -> [u8; 4];

    fn decode_trace_data(
        &self,
        index: u64,
        data: StaticReturnBindings,
        return_data: Bytes,
        from_address: Address,
        target_address: Address,
        msg_sender: Address,
        logs: &Vec<Log>,
        db_tx: &brontes_database::libmdbx::tx::CompressedLibmdbxTx<RO>,
    ) -> Option<Actions>;
}

pub trait FactoryDecoder {
    /// is concat(factory_address, function_selector);
    fn address_and_function_selector(&self) -> [u8; 24];

    fn decode_new_pool<T: TracingProvider>(
        &self,
        tracer: Arc<T>,
        deployed_address: Address,
        parent_calldata: Bytes,
    ) -> impl Future<Output = Vec<DiscoveredPool>> + Send;
}

pub trait FactoryDecoderDispatch: Sync + Send {
    fn dispatch<T: TracingProvider>(
        tracer: Arc<T>,
        factory: Address,
        deployed_address: Address,
        parent_calldata: Bytes,
    ) -> impl Future<Output = Vec<DiscoveredPool>> + Send;
}
