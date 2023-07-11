use reth_db::{
    database::{Database, DatabaseGAT},
    mdbx::{Env, WriteMap},
    tables,
    transaction::DbTx,
    DatabaseError,
};
use reth_network_api::noop::NoopNetwork;
use reth_primitives::MAINNET;
use reth_provider::{providers::BlockchainProvider, ProviderFactory};
use reth_revm::Factory;
use reth_rpc::{
    eth::{
        cache::{EthStateCache, EthStateCacheConfig},
        gas_oracle::{GasPriceOracle, GasPriceOracleConfig},
    },
    DebugApi, EthApi, EthFilter, TraceApi, TracingCallGuard,
};
use reth_tasks::TaskManager;
use reth_transaction_pool::{EthTransactionValidator, GasCostOrdering, Pool, PooledTransaction};
use std::{fmt::Debug, path::Path, sync::Arc};

pub type RethTxPool =
    Pool<EthTransactionValidator<Provider, PooledTransaction>, GasCostOrdering<PooledTransaction>>;

pub struct TracingClient {
    pub api: EthApi<Provider, RethTxPool, NoopNetwork>,
    pub trace: TraceApi<Provider, RethApi>,
}

impl TracingClient {
    pub fn new(db_path: &Path, handle: Handle) -> Self {
        let task_manager = TaskManager::new(handle);
        let task_executor = task_manager.executor();

        tokio::task::spawn(task_manager);

        let chain = MAINNET.clone();
        let db = Arc::new(init_db(db_path).unwrap());

        let tree_externals = TreeExternals::new(
            db.clone(),
            Arc::new(BeaconConsensus::new(Arc::clone(&chain))),
            Factory::new(chain.clone()),
            Arc::clone(&chain),
        );

        let tree_config = BlockchainTreeConfig::default();

        let (canon_state_notification_sender, _receiver) =
            tokio::sync::broadcast::channel(tree_config.max_reorg_depth() as usize * 2);

        let blockchain_tree = ShareableBlockchainTree::new(
            BlockchainTree::new(tree_externals, canon_state_notification_sender, tree_config)
                .unwrap(),
        );

        let provider = BlockchainProvider::new(
            ProviderFactory::new(Arc::clone(&db), Arc::clone(&chain)),
            blockchain_tree,
        )
        .unwrap();

        let state_cache = EthStateCache::spawn(provider.clone(), EthStateCacheConfig::default());

        let tx_pool = reth_transaction_pool::Pool::eth_pool(
            EthTransactionValidator::new(provider.clone(), chain, task_executor.clone()),
            Default::default(),
        );

        let reth_api = EthApi::new(
            provider.clone(),
            tx_pool.clone(),
            NoopNetwork::default(),
            state_cache.clone(),
            GasPriceOracle::new(
                provider.clone(),
                GasPriceOracleConfig::default(),
                state_cache.clone(),
            ),
        );

        let tracing_call_guard = TracingCallGuard::new(10);

        let reth_trace = TraceApi::new(
            provider.clone(),
            reth_api.clone(),
            state_cache.clone(),
            Box::new(task_executor.clone()),
            tracing_call_guard.clone(),
        );

        let reth_debug = DebugApi::new(
            provider.clone(),
            reth_api.clone(),
            Box::new(task_executor.clone()),
            tracing_call_guard,
        );

        let reth_filter =
            EthFilter::new(provider, tx_pool, state_cache, 1000, Box::new(task_executor));

        Self { api: reth_api, trace: reth_trace }
    }
}