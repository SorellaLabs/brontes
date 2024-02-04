use alloy_primitives::TxHash;
use reth_primitives::{BlockId, BlockNumber, BlockNumberOrTag, Bytes, Header, B256};
use reth_rpc_types::{state::StateOverride, BlockOverrides, CallRequest, TransactionReceipt};

use crate::structured_trace::TxTrace;

#[async_trait::async_trait]
#[auto_impl::auto_impl(Box)]
pub trait TracingProvider: Send + Sync + 'static {
    async fn eth_call(
        &self,
        request: CallRequest,
        block_number: Option<BlockId>,
        state_overrides: Option<StateOverride>,
        block_overrides: Option<Box<BlockOverrides>>,
    ) -> eyre::Result<Bytes>;

    async fn block_hash_for_id(&self, block_num: u64) -> eyre::Result<Option<B256>>;

    #[cfg(not(feature = "local"))]
    fn best_block_number(&self) -> eyre::Result<u64>;

    #[cfg(feature = "local")]
    async fn best_block_number(&self) -> eyre::Result<u64>;

    async fn replay_block_transactions(
        &self,
        block_id: BlockId,
    ) -> eyre::Result<Option<Vec<TxTrace>>>;

    async fn block_receipts(
        &self,
        number: BlockNumberOrTag,
    ) -> eyre::Result<Option<Vec<TransactionReceipt>>>;

    async fn header_by_number(&self, number: BlockNumber) -> eyre::Result<Option<Header>>;

    async fn block_and_tx_index(&self, hash: TxHash) -> eyre::Result<(u64, usize)>;
}
