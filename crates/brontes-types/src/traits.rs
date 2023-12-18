use reth_interfaces::provider::ProviderResult;
use reth_primitives::{BlockId, BlockNumber, BlockNumberOrTag, Bytes, Header, B256};
use reth_rpc::eth::error::EthResult;
use reth_rpc_types::{state::StateOverride, BlockOverrides, CallRequest, TransactionReceipt};

use crate::structured_trace::TxTrace;

#[async_trait::async_trait]
#[auto_impl::auto_impl(&, Arc, Box)]
pub trait TracingProvider: Send + Sync + 'static {
    async fn eth_call(
        &self,
        request: CallRequest,
        block_number: Option<BlockId>,
        state_overrides: Option<StateOverride>,
        block_overrides: Option<Box<BlockOverrides>>,
    ) -> ProviderResult<Bytes>;

    async fn block_hash_for_id(&self, block_num: u64) -> ProviderResult<Option<B256>>;

    #[cfg(not(feature = "local"))]
    fn best_block_number(&self) -> ProviderResult<u64>;

    #[cfg(feature = "local")]
    async fn best_block_number(&self) -> ProviderResult<u64>;

    async fn replay_block_transactions(&self, block_id: BlockId)
        -> EthResult<Option<Vec<TxTrace>>>;

    async fn block_receipts(
        &self,
        number: BlockNumberOrTag,
    ) -> ProviderResult<Option<Vec<TransactionReceipt>>>;

    async fn header_by_number(&self, number: BlockNumber) -> ProviderResult<Option<Header>>;
}
