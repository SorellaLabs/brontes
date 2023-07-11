use reth_blockchain_tree::{
    externals::TreeExternals, BlockchainTree, BlockchainTreeConfig, ShareableBlockchainTree,
};
use reth_db::{
    database::{Database, DatabaseGAT},
    mdbx::{Env, WriteMap},
    tables,
    transaction::DbTx,
    DatabaseError,
};

use eyre::Context;
use reth_beacon_consensus::BeaconConsensus;

use reth_network_api::noop::NoopNetwork;
use reth_primitives::MAINNET;
use reth_provider::{providers::BlockchainProvider, ProviderFactory};
use reth_revm::Factory;
use reth_rpc::{
    eth::{
        cache::{EthStateCache, EthStateCacheConfig},
        gas_oracle::{GasPriceOracle, GasPriceOracleConfig},
    },
    EthApi, TraceApi, TracingCallGuard,
};
use reth_tasks::TaskManager;
use reth_transaction_pool::{EthTransactionValidator, GasCostOrdering, Pool, PooledTransaction};
use std::{fmt::Debug, path::Path, sync::Arc};
use tokio::runtime::Handle;

pub type Provider = BlockchainProvider<
    Arc<Env<WriteMap>>,
    ShareableBlockchainTree<Arc<Env<WriteMap>>, Arc<BeaconConsensus>, Factory>,
>;

pub type RethApi = EthApi<Provider, RethTxPool, NoopNetwork>;

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

        let api = EthApi::new(
            provider.clone(),
            tx_pool,
            NoopNetwork::default(),
            state_cache.clone(),
            GasPriceOracle::new(
                provider.clone(),
                GasPriceOracleConfig::default(),
                state_cache.clone(),
            ),
        );

        let tracing_call_guard = TracingCallGuard::new(10);

        let trace = TraceApi::new(
            provider,
            api.clone(),
            state_cache,
            Box::new(task_executor),
            tracing_call_guard,
        );

        Self { api, trace }
    }
}

/// re-implementation of 'view()'
/// allows for a function to be passed in through a RO libmdbx transaction
/// /reth/crates/storage/db/src/abstraction/database.rs
pub fn view<F, T>(db: &Env<WriteMap>, f: F) -> Result<T, DatabaseError>
where
    F: FnOnce(&<Env<WriteMap> as DatabaseGAT<'_>>::TX) -> T,
{
    let tx = db.tx()?;
    let res = f(&tx);
    tx.commit()?;

    Ok(res)
}

/// Opens up an existing database at the specified path.
pub fn init_db<P: AsRef<Path> + Debug>(path: P) -> eyre::Result<Env<WriteMap>> {
    let _ = std::fs::create_dir_all(path.as_ref());
    let db = reth_db::mdbx::Env::<reth_db::mdbx::WriteMap>::open(
        path.as_ref(),
        reth_db::mdbx::EnvKind::RO,
        None,
    )?;

    view(&db, |tx| {
        for table in tables::Tables::ALL.iter().map(|table| table.name()) {
            tx.inner.open_db(Some(table)).wrap_err("Could not open db.").unwrap();
        }
    })?;

    Ok(db)
}
