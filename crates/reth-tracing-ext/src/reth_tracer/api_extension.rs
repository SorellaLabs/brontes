use reth_network_api::noop::NoopNetwork;
use reth_rpc_types::TransactionInfo;
use reth_rpc::EthApi;
use reth_node_ethereum::EthEvmConfig;
use reth_primitives::{BlockId, Block};
use reth_provider::{StateProviderBox, BlockReaderIdExt};
use reth_revm::database::StateProviderDatabase;
use inspector::BrontesTracingInspector;
use jsonrpsee::proc_macros::rpc;
use reth_rpc::eth::error::EthResult;
use brontes_types::structured_trace::TxTrace;
use revm::{
    db::CacheDB,
    primitives::{
        ExecutionResult, State
    }
};

use crate::{Provider, RethTxPool};

use super::inspector;
/// trait interface for a custom rpc namespace: `BrontesRpc`
///
/// This defines an additional namespace where all methods are configured as trait functions.
#[rpc(server, namespace = "brontesrpcExt")]
pub trait BrontesRpcExtApi {
    /// Returns all transaction traces
    #[method(name = "getTxTraces")]
    fn get_tx_traces(&self, block_id: BlockId) -> EthResult<Option<Vec<TxTrace>>>;
}

/// The type that implements `brontesRpc` rpc namespace trait
pub struct BrontesRpcExt<Provider> {
    pub provider: Provider,
}

impl<Provider> BrontesRpcExtApiServer for BrontesRpcExt<Provider>
where
    Provider: BlockReaderIdExt + 'static,
{
    fn get_tx_traces(&self, block_id: BlockId) -> EthResult<Option<Vec<TxTrace>>> 
    {
        unimplemented!()
    }
}