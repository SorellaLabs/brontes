use std::{fmt::Debug, path::{Path, PathBuf}, sync::Arc};

use alloy_primitives::{Log, B256};
use brontes_types::{
    structured_trace::{TransactionTraceWithLogs, TxTrace},
    BrontesTaskExecutor,
};
use reth_beacon_consensus::BeaconConsensus;
use reth_blockchain_tree::{
    externals::TreeExternals, BlockchainTree, BlockchainTreeConfig, ShareableBlockchainTree,
};
use reth_db::DatabaseEnv;
use reth_network_api::noop::NoopNetwork;
use reth_node_ethereum::EthEvmConfig;
use reth_primitives::{BlockId, Bytes, PruneModes, MAINNET, U64};
use reth_provider::{providers::BlockchainProvider, ProviderFactory};
use reth_revm::{
    inspectors::GasInspector,
    revm::{
        interpreter::InstructionResult,
        primitives::{ExecutionResult, SpecId},
    },
    EvmProcessorFactory,
};
use reth_rpc::{
    eth::{
        cache::{EthStateCache, EthStateCacheConfig},
        error::EthResult,
        gas_oracle::{GasPriceOracle, GasPriceOracleConfig},
        EthTransactions, FeeHistoryCache, FeeHistoryCacheConfig, RPC_DEFAULT_GAS_CAP,
    },
    EthApi, TraceApi,
};
use reth_tasks::pool::{BlockingTaskGuard, BlockingTaskPool};
use reth_rpc_types::{trace::parity::*, TransactionInfo};
use reth_tracer::inspector::BrontesTracingInspector;
use reth_transaction_pool::{
    blobstore::NoopBlobStore, validate::EthTransactionValidatorBuilder, CoinbaseTipOrdering,
    EthPooledTransaction, EthTransactionValidator, Pool, TransactionValidationTaskExecutor,
};
use reth_tracer::{arena::CallTraceArena, types::{CallTraceNode, CallKind}, config::{TracingInspectorConfig, StackSnapshotType}};
mod provider;
pub mod reth_tracer;

pub type Provider = BlockchainProvider<
    Arc<DatabaseEnv>,
    ShareableBlockchainTree<Arc<DatabaseEnv>, EvmProcessorFactory<EthEvmConfig>>,
>;

pub type RethApi = EthApi<Provider, RethTxPool, NoopNetwork, EthEvmConfig>;

pub type RethTxPool = Pool<
    TransactionValidationTaskExecutor<EthTransactionValidator<Provider, EthPooledTransaction>>,
    CoinbaseTipOrdering<EthPooledTransaction>,
    NoopBlobStore,
>;

#[derive(Debug, Clone)]
pub struct TracingClient {
    pub api:   EthApi<Provider, RethTxPool, NoopNetwork, EthEvmConfig>,
    pub trace: TraceApi<Provider, RethApi>,
}

impl TracingClient {
    pub fn new_with_db(
        db: Arc<DatabaseEnv>,
        max_tasks: u64,
        task_executor: BrontesTaskExecutor,
    ) -> Self {
        let chain = MAINNET.clone();
        // some breaking changes were introduced in provider factory in the latest reth version, which required to pass the path to the provider factory, for now I have passed an empty path, but this should be fixed in the future see line 73 and 88
        let provider_factory = ProviderFactory::new(Arc::clone(&db), Arc::clone(&chain), PathBuf::new()).unwrap();

        let tree_externals = TreeExternals::new(
            provider_factory,
            Arc::new(BeaconConsensus::new(Arc::clone(&chain))),
            EvmProcessorFactory::new(chain.clone(), EthEvmConfig::default()),
        );

        let tree_config = BlockchainTreeConfig::default();

        let blockchain_tree = ShareableBlockchainTree::new(
            BlockchainTree::new(tree_externals, tree_config, Some(PruneModes::none())).unwrap(),
        );

        let provider = BlockchainProvider::new(
            ProviderFactory::new(Arc::clone(&db), Arc::clone(&chain), PathBuf::new()).unwrap(),
            blockchain_tree,
        )
        .unwrap();

        let state_cache = EthStateCache::spawn_with(
            provider.clone(),
            EthStateCacheConfig::default(),
            task_executor.clone(),
            EthEvmConfig::default(),
        );

        let transaction_validator = EthTransactionValidatorBuilder::new(chain.clone())
            .build_with_tasks(provider.clone(), task_executor.clone(), NoopBlobStore::default());

        let tx_pool = reth_transaction_pool::Pool::eth_pool(
            transaction_validator,
            NoopBlobStore::default(),
            Default::default(),
        );

        let blocking = BlockingTaskPool::build().unwrap();
        let eth_state_config = EthStateCacheConfig::default();
        let fee_history = FeeHistoryCache::new(
            EthStateCache::spawn_with(
                provider.clone(),
                eth_state_config,
                task_executor.clone(),
                EthEvmConfig::default(),
            ),
            FeeHistoryCacheConfig::default(),
        );
        // blocking task pool
        // fee history cache
        let api = EthApi::with_spawner(
            provider.clone(),
            tx_pool.clone(),
            NoopNetwork::default(),
            state_cache.clone(),
            GasPriceOracle::new(
                provider.clone(),
                GasPriceOracleConfig::default(),
                state_cache.clone(),
            ),
            RPC_DEFAULT_GAS_CAP.into(),
            Box::new(task_executor.clone()),
            blocking,
            fee_history,
            EthEvmConfig::default(),
        );

        let tracing_call_guard = BlockingTaskGuard::new((max_tasks as u32).try_into().unwrap());

        let trace = TraceApi::new(provider, api.clone(), tracing_call_guard);

        Self { api, trace }
    }

    pub fn new(db_path: &Path, max_tasks: u64, task_executor: BrontesTaskExecutor) -> Self {
        let db = Arc::new(init_db(db_path).unwrap());
        Self::new_with_db(db, max_tasks, task_executor)
    }

    /// Replays all transactions in a block using a custom inspector for each transaction
pub async fn replay_block_transactions_with_inspector(
    &self,
    block_id: BlockId,
) -> EthResult<Option<Vec<TxTrace>>> {
    let insp_setup = || {
        BrontesTracingInspector {
            config:                TracingInspectorConfig {
                record_logs:              true,
                record_steps:             false,
                record_state_diff:        false,
                record_stack_snapshots:   StackSnapshotType::None,
                record_memory_snapshots:  false,
                record_call_return_data:  true,
                exclude_precompile_calls: true,
            },
            traces:                CallTraceArena::default(),
            trace_stack:           Vec::new(),
            step_stack:            Vec::new(),
            last_call_return_data: None,
            gas_inspector:         GasInspector::default(),
            spec_id:               None,
        }
    };

    self.api
        .trace_block_with_inspector(
            block_id,
            insp_setup,
            move |tx_info, inspector, res, _, _| {

                Ok(inspector.into_trace_results(tx_info, &res))
            },
        )
        .await
}
}

#[derive(Debug, Clone, Copy)]
pub struct StackStep {
    _trace_idx: usize,
    _step_idx:  usize,
}

/// Opens up an existing database at the specified path.
pub fn init_db<P: AsRef<Path> + Debug>(path: P) -> eyre::Result<DatabaseEnv> {
    reth_db::open_db(path.as_ref(), Default::default())
}

#[cfg(all(test, feature = "local-reth"))]
pub mod test {

    use brontes_core::test_utils::TraceLoader;
    use futures::future::join_all;
    use reth_primitives::{BlockId, BlockNumberOrTag};

    #[brontes_macros::test]
    async fn ensure_traces_eq() {
        let block = 18500018;
        let loader = TraceLoader::new().await;
        let tp = loader.tracing_provider.get_tracer();
        let mut traces = join_all((0..20).map(|_| async {
            tp.replay_block_transactions(BlockId::Number(BlockNumberOrTag::Number(block)))
                .await
                .unwrap()
                .unwrap()
        }))
        .await;

        let cmp = traces.pop().unwrap();
        traces
            .into_iter()
            .for_each(|trace| assert_eq!(cmp, trace, "got traces that aren't equal"));
    }
}
