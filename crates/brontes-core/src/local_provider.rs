use std::sync::Arc;

use alloy_provider::{Provider, RootProvider};
use alloy_rpc_types::Log;
use alloy_transport_http::Http;
use brontes_types::{
    structured_trace::TxTrace,
    traits::{LogProvider, TracingProvider},
};
use governor::DefaultDirectRateLimiter;
use reth_primitives::{
    Address, BlockId, BlockNumber, BlockNumberOrTag, Bytecode, Bytes, Header, StorageValue, TxHash,
    B256,
};
use reth_rpc_types::{state::StateOverride, BlockOverrides, Filter, TransactionRequest};

use crate::rpc_client::{RpcClient, TraceOptions};

#[derive(Debug, Clone)]
pub struct LocalProvider {
    provider:   Arc<RootProvider<Http<reqwest::Client>, alloy_network::AnyNetwork>>,
    rpc_client: Arc<RpcClient>,
    retries:    u8,
    limiter:    Option<Arc<DefaultDirectRateLimiter>>,
}

impl LocalProvider {
    pub fn new(url: String, retries: u8) -> Self {
        tracing::info!(target: "brontes", "creating local provider with url: {}", url);

        Self {
            provider: Arc::new(RootProvider::new_http(url.parse().unwrap())),
            rpc_client: Arc::new(RpcClient::new(url.parse().unwrap())),
            retries,
            limiter: None,
        }
    }

    pub fn new_with_limiter(
        url: String,
        retries: u8,
        limiter: Option<Arc<DefaultDirectRateLimiter>>,
    ) -> Self {
        Self {
            provider: Arc::new(RootProvider::new_http(url.parse().unwrap())),
            rpc_client: Arc::new(RpcClient::new(url.parse().unwrap())),
            retries,
            limiter: limiter,
        }
    }
}

#[async_trait::async_trait]
impl LogProvider for LocalProvider {
    async fn block_hash_for_id(&self, block_num: u64) -> eyre::Result<Option<B256>> {
        self.provider
            .get_block(BlockId::Number(BlockNumberOrTag::Number(block_num)), true)
            .await
            .map(|op| op.map(|block| block.header.hash.unwrap()))
            .map_err(Into::into)
    }

    #[cfg(not(feature = "local-reth"))]
    async fn best_block_number(&self) -> eyre::Result<u64> {
        self.provider.get_block_number().await.map_err(Into::into)
    }

    async fn get_logs(&self, filter: &Filter) -> eyre::Result<Vec<Log>> {
        if let Some(limiter) = self.limiter.as_ref() {
            limiter.until_ready().await;
        }

        let res = self.provider.get_logs(filter).await;
        if let Err(e) = res {
            return Err(e.into());
        }

        let logs = res.unwrap();
        Ok(logs)
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
        let any_request = alloy_rpc_types::WithOtherFields::new(request);
        let mut attempts = 0;
        loop {
            let res = self
                .provider
                .call(&any_request, block_number.unwrap_or(BlockId::latest()))
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
                let trace_options = TraceOptions { tracer: "brontesTracer".to_string() };
                let traces = self
                    .rpc_client
                    .debug_trace_block_by_hash(hash.block_hash, trace_options)
                    .await?;
                Ok(Some(traces))
            }
            BlockId::Number(number) => {
                let trace_options = TraceOptions { tracer: "brontesTracer".to_string() };
                if number.is_number() {
                    let traces = self
                        .rpc_client
                        .debug_trace_block_by_number(number.as_number().unwrap(), trace_options)
                        .await?;
                    Ok(Some(traces))
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
    ) -> eyre::Result<Option<Vec<alloy_rpc_types::AnyTransactionReceipt>>> {
        // Get the receipts directly from the provider
        let raw_receipts = self.provider.get_block_receipts(number).await?;
        // Map the result
        Ok(raw_receipts)
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

#[cfg(test)]
mod tests {
    use std::env;

    use alloy_rpc_types::Filter;
    use alloy_sol_macro::sol;
    use alloy_sol_types::SolEvent;
    use tracing_subscriber::{fmt, EnvFilter};

    use super::*;

    sol!(
        #![sol(all_derives)]
        BalancerV2,
        "../brontes-classifier/classifier-abis/balancer/BalancerV2Vault.json"
    );

    sol!(
        #![sol(all_derives)]
        UniswapV2,
        "../brontes-classifier/classifier-abis/UniswapV2Factory.json"
    );
    sol!(
        #![sol(all_derives)]
        UniswapV3,
        "../brontes-classifier/classifier-abis/UniswapV3Factory.json"
    );

    sol!(
        #![sol(all_derives)]
        UniswapV4,
        "../brontes-classifier/classifier-abis/UniswapV4.json"
    );
    sol!(
        #![sol(all_derives)]
        CamelotV3,
        "../brontes-classifier/classifier-abis/Algebra1_9Factory.json"
    );
    sol!(
        #![sol(all_derives)]
        FluidDEX,
        "../brontes-classifier/classifier-abis/fluid/FluidDexFactory.json"
    );

    // Helper function to get RPC URL from environment
    fn get_rpc_url() -> String {
        dotenv::dotenv().ok();
        env::var("ETH_RPC_URL").expect("ETH_RPC_URL must be set for tests")
    }

    fn init_tracing() {
        let _ = fmt()
            .with_env_filter(
                EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()),
            )
            .with_target(false)
            .with_thread_ids(false)
            .with_file(false)
            .with_line_number(false)
            .try_init();
    }

    #[tokio::test]
    async fn test_get_logs_with_address() {
        init_tracing();
        let url = get_rpc_url();
        let provider = LocalProvider::new(url, 3);

        // Create a filter with a specific address
        // Using USDC contract address on Ethereum mainnet as an example
        let addresses = vec![
            brontes_types::constants::UNISWAP_V2_FACTORY_ADDRESS,
            brontes_types::constants::UNISWAP_V3_FACTORY_ADDRESS,
            brontes_types::constants::UNISWAP_V4_FACTORY_ADDRESS,
            brontes_types::constants::BALANCER_V2_VAULT_ADDRESS,
            brontes_types::constants::CAMELOT_V2_FACTORY_ADDRESS,
            brontes_types::constants::CAMELOT_V3_FACTORY_ADDRESS,
            brontes_types::constants::FLUID_DEX_FACTORY_ADDRESS,
            brontes_types::constants::SUSHISWAP_V2_FACTORY_ADDRESS,
            brontes_types::constants::SUSHISWAP_V3_FACTORY_ADDRESS,
            brontes_types::constants::PANCAKESWAP_V2_FACTORY_ADDRESS,
            brontes_types::constants::PANCAKESWAP_V3_FACTORY_ADDRESS,
        ];

        let topics = vec![
            UniswapV2::PairCreated::SIGNATURE_HASH,
            UniswapV3::PoolCreated::SIGNATURE_HASH,
            UniswapV3::PoolCreated::SIGNATURE_HASH,
            UniswapV4::Initialize::SIGNATURE_HASH,
            CamelotV3::Pool::SIGNATURE_HASH,
            FluidDEX::DexT1Deployed::SIGNATURE_HASH,
            BalancerV2::TokensRegistered::SIGNATURE_HASH,
        ];

        let filter = Filter::new()
            .address(addresses)
            .from_block(BlockNumberOrTag::Number(338833846))
            .to_block(BlockNumberOrTag::Number(338843846))
            .event_signature(topics);

        tracing::info!("Fetching logs for DEX factory addresses");
        // Get logs with address filter
        let logs = provider
            .get_logs(&filter)
            .await
            .expect("Failed to get logs");
        tracing::info!("Retrieved {} logs for DEX factory addresses", logs.len());
    }
}
