use std::{
    fmt::Debug,
    path::{Path, PathBuf},
    sync::Arc,
};

use alloy_rpc_types::BlockId;
use brontes_types::{structured_trace::TxTrace, BrontesTaskExecutor};
use rayon::ThreadPoolBuilder;
use reth_chainspec::MAINNET;
use reth_db::{mdbx::DatabaseArguments, DatabaseEnv};
use reth_network_api::noop::NoopNetwork;
use reth_node_ethereum::{BasicBlockExecutorProvider, EthEvmConfig, EthereumNode};
use reth_node_types::NodeTypesWithDBAdapter;
use reth_provider::{
    providers::{BlockchainProvider, StaticFileProvider},
    ProviderFactory,
};
use reth_rpc::{DebugApi, EthApi, EthFilter, TraceApi};
use reth_rpc_eth_api::helpers::Trace;
use reth_rpc_eth_types::{
    EthResult, EthStateCache, EthStateCacheConfig, FeeHistoryCache, FeeHistoryCacheConfig, GasCap,
    GasPriceOracle, GasPriceOracleConfig,
};
use reth_rpc_server_types::constants::{
    DEFAULT_ETH_PROOF_WINDOW, DEFAULT_MAX_SIMULATE_BLOCKS, DEFAULT_PROOF_PERMITS,
};
use reth_tasks::pool::{BlockingTaskGuard, BlockingTaskPool};
use reth_tracer::{
    arena::CallTraceArena,
    config::{StackSnapshotType, TracingInspectorConfig},
    inspector::BrontesTracingInspector,
};
use reth_transaction_pool::{
    blobstore::NoopBlobStore, validate::EthTransactionValidatorBuilder, CoinbaseTipOrdering,
    EthPooledTransaction, EthTransactionValidator, Pool, TransactionValidationTaskExecutor,
};
use revm::inspector::inspectors::GasInspector;
// use revm::inspector::inspectors::GasInspector;

mod provider;
pub mod reth_tracer;

pub type RethProvider = BlockchainProvider<NodeTypesWithDBAdapter<EthereumNode, Arc<DatabaseEnv>>>;
pub type RethProviderFactory =
    ProviderFactory<NodeTypesWithDBAdapter<EthereumNode, Arc<DatabaseEnv>>>;
pub type RethDbProvider =
    BlockchainProvider<NodeTypesWithDBAdapter<EthereumNode, Arc<DatabaseEnv>>>;
pub type RethApi = EthApi<RethProvider, RethTxPool, NoopNetwork, EthEvmConfig>;
pub type RethFilter = EthFilter<RethApi>;
pub type RethTrace = TraceApi<RethApi>;
pub type RethDebug = DebugApi<RethApi, BasicBlockExecutorProvider<EthEvmConfig>>;
pub type RethTxPool = Pool<
    TransactionValidationTaskExecutor<EthTransactionValidator<RethProvider, EthPooledTransaction>>,
    CoinbaseTipOrdering<EthPooledTransaction>,
    NoopBlobStore,
>;

#[derive(Debug, Clone)]
pub struct TracingClient {
    pub api:              RethApi,
    pub trace:            RethTrace,
    pub provider_factory: RethProviderFactory,
}
impl TracingClient {
    pub fn new_with_db(
        db: Arc<DatabaseEnv>,
        max_tasks: u64,
        task_executor: BrontesTaskExecutor,
        static_files_path: PathBuf,
    ) -> Self {
        let chain = MAINNET.clone();
        let static_file_provider =
            StaticFileProvider::read_only(static_files_path.clone(), true).unwrap();

        let provider_factory: ProviderFactory<
            NodeTypesWithDBAdapter<EthereumNode, Arc<DatabaseEnv>>,
        > = ProviderFactory::new(db.clone(), chain.clone(), static_file_provider);

        let provider = BlockchainProvider::new(provider_factory.clone()).unwrap();

        let state_cache = EthStateCache::spawn_with(
            provider.clone(),
            EthStateCacheConfig::default(),
            task_executor.clone(),
        );

        let transaction_validator = EthTransactionValidatorBuilder::new(provider.clone())
            .build_with_tasks(task_executor.clone(), NoopBlobStore::default());

        let tx_pool = reth_transaction_pool::Pool::eth_pool(
            transaction_validator,
            NoopBlobStore::default(),
            Default::default(),
        );

        let api = EthApi::new(
            provider.clone(),
            tx_pool.clone(),
            NoopNetwork::default(),
            state_cache.clone(),
            GasPriceOracle::new(
                provider.clone(),
                GasPriceOracleConfig::default(),
                state_cache.clone(),
            ),
            GasCap::default(),
            DEFAULT_MAX_SIMULATE_BLOCKS,
            DEFAULT_ETH_PROOF_WINDOW,
            BlockingTaskPool::new(ThreadPoolBuilder::new().build().unwrap()),
            FeeHistoryCache::new(FeeHistoryCacheConfig::default()),
            EthEvmConfig::new(chain.clone()),
            DEFAULT_PROOF_PERMITS,
        );

        let tracing_call_guard = BlockingTaskGuard::new(max_tasks as usize);
        let trace = TraceApi::new(api.clone(), tracing_call_guard);

        Self { api, trace, provider_factory }
    }

    pub fn new(db_path: &Path, max_tasks: u64, task_executor: BrontesTaskExecutor) -> Self {
        let db = Arc::new(init_db(db_path).unwrap());
        let mut static_files = db_path.to_path_buf();
        static_files.pop();
        static_files.push("static_files");
        Self::new_with_db(db, max_tasks, task_executor, static_files)
    }

    /// Replays all transactions in a block using a custom inspector for each
    /// transaction
    pub async fn replay_block_transactions_with_inspector(
        &self,
        block_id: BlockId,
    ) -> EthResult<Option<Vec<TxTrace>>> {
        let insp_setup = || BrontesTracingInspector {
            config:                TracingInspectorConfig {
                record_logs:                 true,
                record_steps:                false,
                record_state_diff:           false,
                record_stack_snapshots:      StackSnapshotType::None,
                record_memory_snapshots:     false,
                exclude_precompile_calls:    true,
                record_returndata_snapshots: false,
                record_opcodes_filter:       None,
                record_immediate_bytes:      false,
            },
            traces:                CallTraceArena::default(),
            trace_stack:           Vec::new(),
            step_stack:            Vec::new(),
            last_call_return_data: None,
            gas_inspector:         GasInspector::default(),
            spec_id:               None,
        };

        let t =
            self.api
                .trace_block_inspector(
                    block_id,
                    None,
                    insp_setup,
                    move |tx_info, inspector, res, _, _| {
                        Ok(inspector.into_trace_results(tx_info, &res))
                    },
                )
                .await?;

        Ok(t)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct StackStep {
    _trace_idx: usize,
    _step_idx:  usize,
}

/// Opens up an existing database at the specified path.
pub fn init_db<P: AsRef<Path> + Debug>(path: P) -> eyre::Result<DatabaseEnv> {
    reth_db::open_db_read_only(path.as_ref(), DatabaseArguments::new(Default::default()))
}

#[cfg(all(test, feature = "local-reth"))]
pub mod test {
    use alloy_rpc_types::{BlockId, BlockNumberOrTag};
    use brontes_core::test_utils::TraceLoader;
    use futures::future::join_all;

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

    #[brontes_macros::test]
    async fn ensure_no_failure() {
        let block = 19586294;
        let loader = TraceLoader::new().await;
        let tp = loader.tracing_provider.get_tracer();
        let mut traces = tp
            .replay_block_transactions(BlockId::Number(BlockNumberOrTag::Number(block)))
            .await
            .unwrap()
            .unwrap();
        let res = traces.remove(6);
        let not_broken = res.is_success;
        assert!(not_broken, "shit failed when shouldn't of: {:#?}", res);
    }
}
