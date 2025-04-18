use alloy_primitives::TxHash;
use reth_primitives::{
    Address, BlockId, BlockNumber, BlockNumberOrTag, Bytecode, Bytes, Header, StorageValue, B256,
};
use reth_rpc_types::{state::StateOverride, BlockOverrides, TransactionRequest};

use crate::structured_trace::TxTrace;

#[async_trait::async_trait]
#[auto_impl::auto_impl(Box)]
pub trait TracingProvider: Send + Sync + 'static {
    async fn eth_call(
        &self,
        request: TransactionRequest,
        block_number: Option<BlockId>,
        state_overrides: Option<StateOverride>,
        block_overrides: Option<Box<BlockOverrides>>,
    ) -> eyre::Result<Bytes>;

    /// eth call that fetches state and does minimal processing
    /// will bypass threadpool
    async fn eth_call_light(
        &self,
        request: TransactionRequest,
        block_number: BlockId,
    ) -> eyre::Result<Bytes> {
        self.eth_call(request, Some(block_number), None, None).await
    }

    async fn block_hash_for_id(&self, block_num: u64) -> eyre::Result<Option<B256>>;

    #[cfg(feature = "local-reth")]
    fn best_block_number(&self) -> eyre::Result<u64>;

    #[cfg(not(feature = "local-reth"))]
    async fn best_block_number(&self) -> eyre::Result<u64>;

    async fn replay_block_transactions(
        &self,
        block_id: BlockId,
    ) -> eyre::Result<Option<Vec<TxTrace>>>;

    async fn block_receipts(
        &self,
        number: BlockNumberOrTag,
    ) -> eyre::Result<Option<Vec<alloy_rpc_types::AnyTransactionReceipt>>>;

    async fn header_by_number(&self, number: BlockNumber) -> eyre::Result<Option<Header>>;

    async fn block_and_tx_index(&self, hash: TxHash) -> eyre::Result<(u64, usize)>;

    // DB Access Methods
    async fn get_storage(
        &self,
        block_number: Option<u64>,
        address: Address,
        storage_key: B256,
    ) -> eyre::Result<Option<StorageValue>>;

    async fn get_bytecode(
        &self,
        block_number: Option<u64>,
        address: Address,
    ) -> eyre::Result<Option<Bytecode>>;
}
