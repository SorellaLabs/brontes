use std::sync::Arc;

use alloy_primitives::{Address, BlockNumber, Bytes, StorageValue, TxHash, B256};
use alloy_provider::{Provider, RootProvider};
use alloy_rpc_types::AnyReceiptEnvelope;
use alloy_transport_http::Http;
use brontes_types::{structured_trace::TxTrace, traits::TracingProvider};
use itertools::Itertools;
use reth_primitives::Bytecode;
use reth_rpc_types::{
    state::StateOverride, BlockId, BlockNumberOrTag, BlockOverrides, BlockTransactionsKind, Log,
    TransactionReceipt, TransactionRequest,
};

#[derive(Debug, Clone)]
pub struct LocalProvider {
    provider: Arc<RootProvider<Http<reqwest::Client>>>,
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
                .call(&request.clone())
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
            .get_block(
                BlockId::Number(BlockNumberOrTag::Number(block_num)),
                BlockTransactionsKind::Full,
            )
            .await
            .map(|op| op.map(|block| block.header.hash.unwrap()))
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
    ) -> eyre::Result<Option<Vec<TransactionReceipt<AnyReceiptEnvelope<Log>>>>> {
        Ok(self.provider.get_block_receipts(number).await?.map(|t| {
            t.into_iter()
                .map(|tx| {
                    tx.map_inner(|reciept_env| {
                        let bloom = reciept_env.as_receipt_with_bloom().unwrap().clone();
                        let log_type = reciept_env.tx_type() as u8;
                        AnyReceiptEnvelope { inner: bloom, r#type: log_type }
                    })
                })
                .collect_vec()
        }))
    }

    async fn block_and_tx_index(&self, hash: TxHash) -> eyre::Result<(u64, usize)> {
        let err = || eyre::eyre!("failed to unwrap option");
        let tx = self
            .provider
            .get_transaction_by_hash(hash)
            .await?
            .ok_or(err())?;

        Ok((tx.block_number.ok_or_else(err)?, tx.transaction_index.ok_or_else(err)? as usize))
    }

    async fn header_by_number(
        &self,
        number: BlockNumber,
    ) -> eyre::Result<Option<reth_rpc_types::Header>> {
        let err = || eyre::eyre!("failed to unwrap option");
        let block = self
            .provider
            .get_block(
                BlockId::Number(BlockNumberOrTag::Number(number)),
                BlockTransactionsKind::Full,
            )
            .await?
            .ok_or_else(err)?;

        // let inner = block.header;
        // let header = reth_rpc_types::Header {
        //     hash: inner.hash,
        //     parent_hash: inner.parent_hash,
        //     uncles_hash: inner.parent_hash,
        //     miner: inner.miner,
        //     state_root: inner.state_root,
        //     transactions_root: inner.transactions_root,
        //     receipts_root: inner.receipts_root,
        //     logs_bloom: inner.logs_bloom,
        //     difficulty: inner.difficulty,
        //     number: inner.number,
        //     gas_limit: inner.gas_limit as u128,
        //     gas_used: inner.gas_used as u128,
        //     timestamp: inner.timestamp,
        //     total_difficulty: Some(inner.difficulty),
        //     extra_data: inner.extra_data,
        //     mix_hash: inner.mix_hash,
        //     nonce: inner.nonce,
        //     base_fee_per_gas: inner.base_fee_per_gas.map(|v| v as u128),
        //     withdrawals_root: inner.withdrawals_root,
        //     blob_gas_used: inner.blob_gas_used.map(|v| v as u128),
        //     excess_blob_gas: inner.excess_blob_gas.map(|v| v as u128),
        //     parent_beacon_block_root: inner.parent_beacon_block_root,
        //     requests_root: inner.requests_root,
        // };

        Ok(Some(block.header))
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
