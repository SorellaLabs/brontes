use brontes_types::{structured_trace::TxTrace, traits::TracingProvider};
use reth_interfaces::provider::ProviderResult;
use reth_primitives::{BlockId, BlockNumber, BlockNumberOrTag, Bytes, Header, B256};
use reth_provider::{BlockIdReader, BlockNumReader, HeaderProvider};
use reth_rpc::eth::error::EthResult;
use reth_rpc_api::{EthApiServer, EthFilterApiServer};
use reth_rpc_types::{
    state::StateOverride, BlockOverrides, CallRequest, Filter, Log, TransactionReceipt,
};

use crate::TracingClient;

#[async_trait::async_trait]
impl TracingProvider for TracingClient {
    async fn eth_call(
        &self,
        request: CallRequest,
        block_number: Option<BlockId>,
        state_overrides: Option<StateOverride>,
        block_overrides: Option<Box<BlockOverrides>>,
    ) -> ProviderResult<Bytes> {
        // NOTE: these types are equivalent, however we want ot
        EthApiServer::call(&self.api, request, block_number, state_overrides, block_overrides)
            .await
            .map_err(|_| reth_provider::ProviderError::StateRootNotAvailableForHistoricalBlock)
    }

    async fn block_hash_for_id(&self, block_num: u64) -> ProviderResult<Option<B256>> {
        self.trace
            .provider()
            .block_hash_for_id(BlockId::Number(BlockNumberOrTag::Number(block_num)))
    }

    #[cfg(not(feature = "local"))]
    fn best_block_number(&self) -> ProviderResult<u64> {
        self.trace.provider().best_block_number()
    }

    #[cfg(feature = "local")]
    async fn best_block_number(&self) -> ProviderResult<u64> {
        self.trace.provider().best_block_number()
    }

    async fn replay_block_transactions(
        &self,
        block_id: BlockId,
    ) -> EthResult<Option<Vec<TxTrace>>> {
        self.replay_block_transactions(block_id).await
    }

    async fn block_receipts(
        &self,
        number: BlockNumberOrTag,
    ) -> ProviderResult<Option<Vec<TransactionReceipt>>> {
        Ok(Some(
            self.api
                .block_receipts(BlockId::Number(number))
                .await
                .unwrap()
                .unwrap(),
        ))
    }

    async fn header_by_number(&self, number: BlockNumber) -> ProviderResult<Option<Header>> {
        self.trace.provider().header_by_number(number)
    }

    async fn logs_from_filter(&self, filter: Filter) -> ProviderResult<Vec<Log>> {
        EthFilterApiServer::logs(&self.filter, filter)
            .await
            .map_err(|_| reth_provider::ProviderError::StateRootNotAvailableForHistoricalBlock)
    }
}
