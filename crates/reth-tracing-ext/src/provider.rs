use brontes_types::{structured_trace::TxTrace, traits::TracingProvider};
use eyre::eyre;
use reth_primitives::{
    Address, BlockId, BlockNumber, BlockNumberOrTag, Bytes, Header, TxHash, B256, U256,
};
use reth_provider::{BlockIdReader, BlockNumReader, HeaderProvider};
use reth_rpc_api::EthApiServer;
use reth_rpc_types::{
    state::StateOverride, BlockOverrides, TransactionReceipt, TransactionRequest,
};

use crate::TracingClient;

#[async_trait::async_trait]
impl TracingProvider for TracingClient {
    async fn eth_call(
        &self,
        request: TransactionRequest,
        block_number: Option<BlockId>,
        state_overrides: Option<StateOverride>,
        block_overrides: Option<Box<BlockOverrides>>,
    ) -> eyre::Result<Bytes> {
        // NOTE: these types are equivalent, however we want ot
        EthApiServer::call(&self.api, request, block_number, state_overrides, block_overrides)
            .await
            .map_err(Into::into)
    }

    async fn block_hash_for_id(&self, block_num: u64) -> eyre::Result<Option<B256>> {
        self.trace
            .provider()
            .block_hash_for_id(BlockId::Number(BlockNumberOrTag::Number(block_num)))
            .map_err(Into::into)
    }

    #[cfg(not(feature = "local"))]
    fn best_block_number(&self) -> eyre::Result<u64> {
        self.trace
            .provider()
            .best_block_number()
            .map_err(Into::into)
    }

    #[cfg(feature = "local")]
    async fn best_block_number(&self) -> eyre::Result<u64> {
        self.trace
            .provider()
            .best_block_number()
            .map_err(Into::into)
    }

    async fn replay_block_transactions(
        &self,
        block_id: BlockId,
    ) -> eyre::Result<Option<Vec<TxTrace>>> {
        self.replay_block_transactions(block_id)
            .await
            .map_err(Into::into)
    }

    async fn block_receipts(
        &self,
        number: BlockNumberOrTag,
    ) -> eyre::Result<Option<Vec<TransactionReceipt>>> {
        self.api
            .block_receipts(BlockId::Number(number))
            .await
            .map_err(Into::into)
    }

    async fn block_and_tx_index(&self, hash: TxHash) -> eyre::Result<(u64, usize)> {
        let Some(tx) = self.api.transaction_by_hash(hash).await? else {
            return Err(eyre!("no transaction found"));
        };

        Ok((tx.block_number.unwrap().to::<u64>(), tx.transaction_index.unwrap().to::<usize>()))
    }

    async fn header_by_number(&self, number: BlockNumber) -> eyre::Result<Option<Header>> {
        self.trace
            .provider()
            .header_by_number(number)
            .map_err(Into::into)
    }

    async fn get_balance(
        &self,
        user: Address,
        block_number: Option<BlockId>,
    ) -> eyre::Result<U256> {
        EthApiServer::balance(&self.api, user, block_number)
            .await
            .map_err(Into::into)
    }
}
