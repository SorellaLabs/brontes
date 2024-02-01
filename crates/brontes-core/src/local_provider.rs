use std::sync::Arc;

use alloy_providers::provider::{Provider, TempProvider};
use alloy_transport_http::Http;
use brontes_types::{structured_trace::TxTrace, traits::TracingProvider};
use reth_primitives::{BlockId, BlockNumber, BlockNumberOrTag, Bytes, Header, TxHash, B256};
use reth_rpc_types::{state::StateOverride, BlockOverrides, CallRequest, TransactionReceipt};

#[derive(Debug, Clone)]
pub struct LocalProvider {
    provider: Arc<Provider<Http<reqwest::Client>>>,
}

impl LocalProvider {
    pub fn new(url: String) -> Self {
        let http = Http::new(url.parse().unwrap());
        Self { provider: Arc::new(Provider::new(http)) }
    }
}

#[async_trait::async_trait]
impl TracingProvider for LocalProvider {
    async fn eth_call(
        &self,
        request: CallRequest,
        block_number: Option<BlockId>,
        state_overrides: Option<StateOverride>,
        block_overrides: Option<Box<BlockOverrides>>,
    ) -> eyre::Result<Bytes> {
        if state_overrides.is_some() || block_overrides.is_some() {
            panic!("local provider doesn't support block or state overrides");
        }
        self.provider
            .call(request, block_number)
            .await
            .map_err(Into::into)
    }

    async fn block_hash_for_id(&self, block_num: u64) -> eyre::Result<Option<B256>> {
        self.provider
            .get_block(BlockId::Number(BlockNumberOrTag::Number(block_num)), true)
            .await
            .map(|op| op.map(|block| block.header.hash.unwrap()))
            .map_err(Into::into)
    }

    #[cfg(not(feature = "local"))]
    fn best_block_number(&self) -> eyre::Result<u64> {
        unreachable!("local provider should only be used with local feature flag")
    }

    #[cfg(feature = "local")]
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
        self.provider
            .get_block_receipts(number)
            .await
            .map_err(Into::into)
    }

    async fn block_and_tx_index(&self, hash: TxHash) -> eyre::Result<(u64, usize)> {
        let tx = self.provider.get_transaction_by_hash(hash).await?;
        let err = || eyre::eyre!("failed to unwrap option");

        Ok((
            tx.block_number.ok_or_else(err)?.to::<u64>(),
            tx.transaction_index.ok_or_else(err)?.to::<usize>(),
        ))
    }

    async fn header_by_number(&self, number: BlockNumber) -> eyre::Result<Option<Header>> {
        let err = || eyre::eyre!("failed to unwrap option");
        let block = self
            .provider
            .get_block(BlockId::Number(BlockNumberOrTag::Number(number)), true)
            .await?
            .ok_or_else(err)?;

        let header = Header {
            number:                   block.header.number.ok_or_else(err)?.to::<u64>(),
            base_fee_per_gas:         block.header.base_fee_per_gas.map(|i| i.to::<u64>()),
            mix_hash:                 block.header.mix_hash.ok_or_else(err)?,
            withdrawals_root:         block.header.withdrawals_root,
            parent_beacon_block_root: block.header.parent_beacon_block_root,
            nonce:                    block
                .header
                .nonce
                .map(|i| u64::from_be_bytes(*i))
                .ok_or_else(err)?,
            gas_used:                 block.header.gas_used.to::<u64>(),
            gas_limit:                block.header.gas_limit.to::<u64>(),
            timestamp:                block.header.timestamp.to::<u64>(),
            difficulty:               block.header.difficulty,
            state_root:               block.header.state_root,
            parent_hash:              block.header.parent_hash,
            receipts_root:            block.header.receipts_root,
            transactions_root:        block.header.transactions_root,
            logs_bloom:               block.header.logs_bloom,
            extra_data:               block.header.extra_data,
            blob_gas_used:            block.header.blob_gas_used.map(|i| i.to::<u64>()),
            excess_blob_gas:          block.header.excess_blob_gas.map(|i| i.to::<u64>()),
            ommers_hash:              block.header.uncles_hash,
            beneficiary:              block.header.miner,
        };

        Ok(Some(header))
    }
}
