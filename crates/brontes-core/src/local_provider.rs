use std::sync::Arc;

use alloy_provider::{debug::DebugApi, Provider, RootProvider};
use alloy_rpc_types::AnyReceiptEnvelope;
use alloy_transport_http::Http;
use brontes_types::{structured_trace::TxTrace, traits::TracingProvider};
use itertools::Itertools;
use reth_primitives::{
    Address, BlockId, BlockNumber, BlockNumberOrTag, Bytecode, Bytes, Header, StorageValue, TxHash,
    B256,
};
use reth_rpc_types::{
    state::StateOverride,
    BlockOverrides, Log, TransactionReceipt, TransactionRequest,
};

use crate::rpc_client::{RpcClient, TraceOptions};

#[derive(Debug, Clone)]
pub struct LocalProvider {
    provider:   Arc<RootProvider<Http<reqwest::Client>>>,
    rpc_client: Arc<RpcClient>,
    retries:    u8,
}

impl LocalProvider {
    pub fn new(url: String, retries: u8) -> Self {
        tracing::info!(target: "brontes", "creating local provider with url: {}", url);

        Self {
            provider: Arc::new(RootProvider::new_http(url.parse().unwrap())),
            rpc_client: Arc::new(RpcClient::new(url.parse().unwrap())),
            retries,
        }
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
                .call(&request.clone(), block_number.unwrap_or(BlockId::latest()))
                .await;
            if res.is_ok() || attempts > self.retries {
                return res.map_err(Into::into)
            }
            attempts += 1
        }
    }

    async fn block_hash_for_id(&self, block_num: u64) -> eyre::Result<Option<B256>> {
        self.provider
            .get_block(BlockId::Number(BlockNumberOrTag::Number(block_num)), true)
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

    async fn replay_block_transactions(
        &self,
        block_id: BlockId,
    ) -> eyre::Result<Option<Vec<TxTrace>>> {
        tracing::info!(target: "brontes", "replaying block transactions: {:?}", block_id);
        match block_id {
            BlockId::Hash(hash) => {
                let trace_options = TraceOptions { tracer: "callTracer".to_string() };
                let trace = self
                    .rpc_client
                    .debug_trace_block_by_hash(hash.block_hash, trace_options)
                    .await?;
                tracing::info!(target: "brontes", "replayed block transactions: {:?}", trace);
                Ok(Some(vec![trace]))
            }
            BlockId::Number(number) => {
                let trace_options = TraceOptions { tracer: "callTracer".to_string() };
                if number.is_number() {
                    let trace = self
                        .rpc_client
                        .debug_trace_block_by_number(number.as_number().unwrap(), trace_options)
                    .await?;
                        Ok(Some(vec![trace]))
                } else {
                    tracing::error!(target: "brontes", "number is not a numeric: {:?}", number);
                    Ok(None)
                }
            }
        }
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
        let tx = self.provider.get_transaction_by_hash(hash).await?;
        let err = || eyre::eyre!("failed to unwrap option");

        Ok((tx.block_number.ok_or_else(err)?, tx.transaction_index.ok_or_else(err)? as usize))
    }

    async fn header_by_number(&self, number: BlockNumber) -> eyre::Result<Option<Header>> {
        let err = || eyre::eyre!("failed to unwrap option");
        let block = self
            .provider
            .get_block(BlockId::Number(BlockNumberOrTag::Number(number)), true)
            .await?
            .ok_or_else(err)?;

        let header = Header {
            number:                   block.header.number.ok_or_else(err)?,
            base_fee_per_gas:         block.header.base_fee_per_gas.map(|f| f as u64),
            mix_hash:                 block.header.mix_hash.ok_or_else(err)?,
            withdrawals_root:         block.header.withdrawals_root,
            parent_beacon_block_root: block.header.parent_beacon_block_root,
            nonce:                    block
                .header
                .nonce
                .map(|i| u64::from_be_bytes(*i))
                .ok_or_else(err)?,
            gas_used:                 block.header.gas_used as u64,
            gas_limit:                block.header.gas_limit as u64,
            timestamp:                block.header.timestamp,
            difficulty:               block.header.difficulty,
            state_root:               block.header.state_root,
            parent_hash:              block.header.parent_hash,
            receipts_root:            block.header.receipts_root,
            transactions_root:        block.header.transactions_root,
            logs_bloom:               block.header.logs_bloom,
            extra_data:               block.header.extra_data,
            blob_gas_used:            block.header.blob_gas_used.map(|f| f as u64),
            excess_blob_gas:          block.header.excess_blob_gas.map(|f| f as u64),
            ommers_hash:              block.header.uncles_hash,
            beneficiary:              block.header.miner,
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
            .get_storage_at(address, storage_key.into(), block_id)
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
        let bytes = self.provider.get_code_at(address, block_id).await?;

        let bytecode = Bytecode::new_raw(bytes);
        Ok(Some(bytecode))
    }
}
