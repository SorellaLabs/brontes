use std::sync::Arc;

use alloy_consensus::Header;
use alloy_primitives::{Address, BlockNumber, Bytes, StorageValue, TxHash, B256};
use alloy_provider::{Provider, RootProvider};
use alloy_rpc_types::{
    state::StateOverride, BlockId, BlockNumberOrTag, BlockOverrides, TransactionReceipt,
    TransactionRequest,
};
use brontes_types::{structured_trace::TxTrace, traits::TracingProvider};
use reth_primitives::Bytecode;

#[derive(Clone)]
pub struct LocalProvider {
    provider: Arc<RootProvider>,
    retries:  u8,
}

impl LocalProvider {
    pub fn new(url: String, retries: u8) -> Self {
        Self { provider: Arc::new(RootProvider::new_http(url.parse().unwrap())), retries }
    }
}

#[async_trait::async_trait]
impl TracingProvider for LocalProvider {
    async fn eth_call(
        &self,
        request: TransactionRequest,
        block_number: Option<BlockId>,
        state_overrides: Option<StateOverride>,
        block_overrides: Option<Box<BlockOverrides>>,
    ) -> eyre::Result<Bytes> {
        if state_overrides.is_some() || block_overrides.is_some() {
            panic!("local provider doesn't support block or state overrides");
        }
        // for tests, shit can get beefy
        let mut attempts = 0;
        loop {
            let res = self
                .provider
                .call(request.clone())
                .block(block_number.unwrap_or(BlockId::latest()))
                .await;
            if res.is_ok() || attempts > self.retries {
                return res.map_err(Into::into);
            }
            attempts += 1
        }
    }

    async fn block_hash_for_id(&self, block_num: u64) -> eyre::Result<Option<B256>> {
        self.provider
            .get_block(BlockId::Number(BlockNumberOrTag::Number(block_num)))
            .full()
            .await
            .map(|op| op.map(|block| block.header.hash))
            .map_err(Into::into)
    }

    #[cfg(feature = "local-reth")]
    fn best_block_number(&self) -> eyre::Result<u64> {
        unreachable!("local provider should only be used with local feature flag")
    }

    #[cfg(not(feature = "local-reth"))]
    async fn best_block_number(&self) -> eyre::Result<u64> {
        self.provider.get_block_number().await.map_err(Into::into)
    }

    async fn replay_block_transactions(&self, _: BlockId) -> eyre::Result<Option<Vec<TxTrace>>> {
        unreachable!(
            "Currently we use a custom tracing model which does not allow for 
                     a local trace to occur"
        );
    }

    async fn block_receipts(
        &self,
        number: BlockNumberOrTag,
    ) -> eyre::Result<Option<Vec<TransactionReceipt>>> {
        Ok(self.provider.get_block_receipts(number.into()).await?)
    }

    async fn block_and_tx_index(&self, hash: TxHash) -> eyre::Result<(u64, usize)> {
        let tx = self
            .provider
            .get_transaction_by_hash(hash)
            .await?
            .ok_or(eyre::eyre!("could not find tx '{hash:?}'"))?;
        let err = || eyre::eyre!("failed to unwrap option");

        Ok((tx.block_number.ok_or_else(err)?, tx.transaction_index.ok_or_else(err)? as usize))
    }

    async fn header_by_number(&self, number: BlockNumber) -> eyre::Result<Option<Header>> {
        let err = || eyre::eyre!("failed to unwrap option");
        let block = self
            .provider
            .get_block(BlockId::Number(BlockNumberOrTag::Number(number)))
            .full()
            .await?
            .ok_or_else(err)?;

        let header = Header {
            requests_hash:            block.header.requests_hash,
            number:                   block.header.number,
            base_fee_per_gas:         block.header.base_fee_per_gas,
            mix_hash:                 block.header.mix_hash,
            withdrawals_root:         block.header.withdrawals_root,
            parent_beacon_block_root: block.header.parent_beacon_block_root,
            nonce:                    block.header.nonce,
            gas_used:                 block.header.gas_used as u64,
            gas_limit:                block.header.gas_limit as u64,
            timestamp:                block.header.timestamp,
            difficulty:               block.header.difficulty,
            state_root:               block.header.state_root,
            parent_hash:              block.header.parent_hash,
            receipts_root:            block.header.receipts_root,
            transactions_root:        block.header.transactions_root,
            logs_bloom:               block.header.logs_bloom,
            extra_data:               block.header.extra_data.clone(),
            blob_gas_used:            block.header.blob_gas_used,
            excess_blob_gas:          block.header.excess_blob_gas,
            ommers_hash:              block.header.ommers_hash,
            beneficiary:              block.header.beneficiary,
        };

        Ok(Some(header))
    }

    async fn get_storage(
        &self,
        block_number: Option<u64>,
        address: Address,
        storage_key: B256,
    ) -> eyre::Result<Option<StorageValue>> {
        let block_id = match block_number {
            Some(number) => BlockId::Number(BlockNumberOrTag::Number(number)),
            None => BlockId::Number(BlockNumberOrTag::Latest),
        };
        let storage_value = self
            .provider
            .get_storage_at(address, storage_key.into())
            .block_id(block_id)
            .await?;

        Ok(Some(storage_value))
    }

    async fn get_bytecode(
        &self,
        block_number: Option<u64>,
        address: Address,
    ) -> eyre::Result<Option<Bytecode>> {
        let block_id = match block_number {
            Some(number) => BlockId::Number(BlockNumberOrTag::Number(number)),
            None => BlockId::Number(BlockNumberOrTag::Latest),
        };
        let bytes = self
            .provider
            .get_code_at(address)
            .block_id(block_id)
            .await?;

        let bytecode = Bytecode::new_raw(bytes);
        Ok(Some(bytecode))
    }
}
