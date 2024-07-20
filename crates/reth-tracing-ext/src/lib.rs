use std::{
    fmt::Debug,
    path::{Path, PathBuf},
    sync::Arc,
};

use brontes_types::{structured_trace::TxTrace, BrontesTaskExecutor};
use reth_beacon_consensus::EthBeaconConsensus;
use reth_blockchain_tree::{
    externals::TreeExternals, BlockchainTree, BlockchainTreeConfig, ShareableBlockchainTree,
};
use reth_db::{mdbx::DatabaseArguments, DatabaseEnv};
use reth_network_api::noop::NoopNetwork;
use reth_node_ethereum::{EthEvmConfig, EthExecutorProvider};
use reth_chainspec::MAINNET;
use reth_rpc_server_types::constants::{DEFAULT_ETH_PROOF_WINDOW, DEFAULT_PROOF_PERMITS};
use reth_primitives::{constants::*, BlockId};
use reth_provider::{
    providers::{BlockchainProvider, StaticFileProvider},
    ProviderFactory,
};
use reth_rpc_eth_api::helpers::Trace;
use reth_prune_types::PruneModes;
use reth_revm::inspectors::GasInspector;
use reth_rpc_eth_types::{
    EthResult, EthStateCache, EthStateCacheConfig, GasPriceOracle, GasPriceOracleConfig, FeeHistoryCache, FeeHistoryCacheConfig
};
use reth_rpc::{ EthApi, TraceApi };
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
mod provider;
pub mod reth_tracer;

pub type Provider = BlockchainProvider<
    Arc<DatabaseEnv>,
    //ShareableBlockchainTree<Arc<DatabaseEnv>, EvmProcessorFactory<EthEvmConfig>>,
>;

pub type RethApi = EthApi<Provider, RethTxPool, NoopNetwork, EthEvmConfig>;

pub type RethTxPool = Pool<
    TransactionValidationTaskExecutor<EthTransactionValidator<Provider, EthPooledTransaction>>,
    CoinbaseTipOrdering<EthPooledTransaction>,
    NoopBlobStore,
>;

#[derive(Debug, Clone)]
pub struct TracingClient {
    pub api:              EthApi<Provider, RethTxPool, NoopNetwork, EthEvmConfig>,
    pub trace:            TraceApi<Provider, RethApi>,
    pub provider_factory: ProviderFactory<Arc<DatabaseEnv>>,
}
impl TracingClient {
    pub fn new_with_db(
        db: Arc<DatabaseEnv>,
        max_tasks: u64,
        task_executor: BrontesTaskExecutor,
        static_files_path: PathBuf,
    ) -> Self {
        let chain = MAINNET.clone();
        let msg = format!("could not make 'StaticFileProvider' at '{}'", static_files_path.display());
        let provider_factory = ProviderFactory::new(
            Arc::clone(&db),
            Arc::clone(&chain),
            StaticFileProvider::read_only(static_files_path).expect(&msg),
        );

        let tree_externals = TreeExternals::new(
            provider_factory.clone(),
            Arc::new(EthBeaconConsensus::new(Arc::clone(&chain))),
            EthExecutorProvider::ethereum(chain.clone()),
        );

        let tree_config = BlockchainTreeConfig::default();

        let blockchain_tree = ShareableBlockchainTree::new(
            BlockchainTree::new(tree_externals, tree_config, PruneModes::none()).unwrap(),
        );

        let provider =
            BlockchainProvider::new(provider_factory.clone(), Arc::new(blockchain_tree)).unwrap();

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
            ETHEREUM_BLOCK_GAS_LIMIT,
            DEFAULT_ETH_PROOF_WINDOW,
            blocking,
            fee_history,
            EthEvmConfig::default(),
            None,
            DEFAULT_PROOF_PERMITS,
        );

        let tracing_call_guard = BlockingTaskGuard::new(max_tasks as usize);
        let trace = TraceApi::new(provider, api.clone(), tracing_call_guard);

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
        };

        self.api
            .trace_block_inspector(block_id, insp_setup, move |tx_info, inspector, res, _, _| {
                Ok(inspector.into_trace_results(tx_info, &res))
            })
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
    reth_db::open_db_read_only(path.as_ref(), DatabaseArguments::new(Default::default()))
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
