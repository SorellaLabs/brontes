use brontes_types::{structured_trace::TxTrace, traits::TracingProvider};
use eyre::eyre;
use reth_primitives::{
    Address, BlockId, BlockNumber, BlockNumberOrTag, Bytecode, Bytes, Header, StorageValue, TxHash,
    B256,
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

    #[cfg(feature = "local-reth")]
    fn best_block_number(&self) -> eyre::Result<u64> {
        self.trace
            .provider()
            .last_block_number()
            .map_err(Into::into)
    }

    #[cfg(not(feature = "local-reth"))]
    async fn best_block_number(&self) -> eyre::Result<u64> {
        self.trace
            .provider()
            .last_block_number()
            .map_err(Into::into)
    }

    async fn replay_block_transactions(
        &self,
        block_id: BlockId,
    ) -> eyre::Result<Option<Vec<TxTrace>>> {
        self.replay_block_transactions_with_inspector(block_id)
            .await
            .map_err(Into::into)
    }

    async fn block_receipts(
        &self,
        number: BlockNumberOrTag,
    ) -> eyre::Result<Option<Vec<TransactionReceipt>>> {
        Ok(self
            .api
            .block_receipts(BlockId::Number(number))
            .await?
            .map(|t| t.into_iter().map(|t| t.inner).collect::<Vec<_>>()))
    }

    async fn block_and_tx_index(&self, hash: TxHash) -> eyre::Result<(u64, usize)> {
        let Some(tx) = self.api.transaction_by_hash(hash).await? else {
            return Err(eyre!("no transaction found"));
        };

        Ok((tx.block_number.unwrap(), tx.transaction_index.unwrap() as usize))
    }

    async fn header_by_number(&self, number: BlockNumber) -> eyre::Result<Option<Header>> {
        self.trace
            .provider()
            .header_by_number(number)
            .map_err(Into::into)
    }

    // DB Access Methods
    async fn get_storage(
        &self,
        block_number: Option<u64>,
        address: Address,
        storage_key: B256,
    ) -> eyre::Result<Option<StorageValue>> {
        let provider = match block_number {
            Some(block_number) => self.provider_factory.history_by_block_number(block_number),
            None => self.provider_factory.latest(),
        }?;

        let storage_value = provider.storage(address, storage_key)?;

        Ok(storage_value)
    }

    async fn get_bytecode(
        &self,
        block_number: Option<u64>,
        address: Address,
    ) -> eyre::Result<Option<Bytecode>> {
        let provider = match block_number {
            Some(block_number) => self.provider_factory.history_by_block_number(block_number),
            None => self.provider_factory.latest(),
        }?;

        let bytecode = provider.account_code(address)?;

        Ok(bytecode)
    }
}
