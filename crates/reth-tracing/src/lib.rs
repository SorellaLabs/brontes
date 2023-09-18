use std::{fmt::Debug, path::Path, sync::Arc};

use eyre::Context;
use reth_beacon_consensus::BeaconConsensus;
use reth_blockchain_tree::{
    externals::TreeExternals, BlockchainTree, BlockchainTreeConfig, ShareableBlockchainTree
};
use reth_db::{
    database::{Database, DatabaseGAT},
    mdbx::{Env, WriteMap},
    tables,
    transaction::DbTx,
    DatabaseError
};
use reth_network_api::noop::NoopNetwork;
use reth_primitives::MAINNET;
use reth_provider::{providers::BlockchainProvider, ProviderFactory};
use reth_revm::Factory;
use reth_rpc::{
    eth::{
        cache::{EthStateCache, EthStateCacheConfig},
        gas_oracle::{GasPriceOracle, GasPriceOracleConfig},
        RPC_DEFAULT_GAS_CAP
    },
    EthApi, TraceApi, TracingCallGuard, TracingCallPool
};
use reth_tasks::TaskManager;
use reth_transaction_pool::{
    blobstore::NoopBlobStore, validate::EthTransactionValidatorBuilder, CoinbaseTipOrdering,
    EthPooledTransaction, EthTransactionValidator, Pool, PoolTransaction, TransactionOrdering,
    TransactionValidationTaskExecutor
};
use tokio::runtime::Handle;

pub type Provider = BlockchainProvider<
    Arc<Env<WriteMap>>,
    ShareableBlockchainTree<Arc<Env<WriteMap>>, Arc<BeaconConsensus>, Factory>
>;

pub type RethApi = EthApi<Provider, RethTxPool, NoopNetwork>;

pub type RethTxPool = Pool<
    TransactionValidationTaskExecutor<EthTransactionValidator<Provider, EthPooledTransaction>>,
    CoinbaseTipOrdering<EthPooledTransaction>,
    NoopBlobStore
>;

pub struct TracingClient {
    pub api:   EthApi<Provider, RethTxPool, NoopNetwork>,
    pub trace: TraceApi<Provider, RethApi>
}

impl TracingClient {
    pub fn new(db_path: &Path, handle: Handle) -> Self {
        let task_manager = TaskManager::new(handle);
        let task_executor: reth_tasks::TaskExecutor = task_manager.executor();

        tokio::task::spawn(task_manager);

        let chain = MAINNET.clone();
        let db = Arc::new(init_db(db_path).unwrap());

        let tree_externals = TreeExternals::new(
            db.clone(),
            Arc::new(BeaconConsensus::new(Arc::clone(&chain))),
            Factory::new(chain.clone()),
            Arc::clone(&chain)
        );

        let tree_config = BlockchainTreeConfig::default();

        let (canon_state_notification_sender, _receiver) =
            tokio::sync::broadcast::channel(tree_config.max_reorg_depth() as usize * 2);

        let blockchain_tree = ShareableBlockchainTree::new(
            BlockchainTree::new(tree_externals, canon_state_notification_sender, tree_config)
                .unwrap()
        );

        let provider = BlockchainProvider::new(
            ProviderFactory::new(Arc::clone(&db), Arc::clone(&chain)),
            blockchain_tree
        )
        .unwrap();

        let state_cache = EthStateCache::spawn(provider.clone(), EthStateCacheConfig::default());

        let transaction_validator = EthTransactionValidatorBuilder::new(chain.clone())
            .build_with_tasks(provider.clone(), task_executor.clone(), NoopBlobStore::default());

        let tx_pool = reth_transaction_pool::Pool::eth_pool(
            transaction_validator,
            NoopBlobStore::default(),
            Default::default()
        );

        let api = EthApi::new(
            provider.clone(),
            tx_pool,
            NoopNetwork::default(),
            state_cache.clone(),
            GasPriceOracle::new(
                provider.clone(),
                GasPriceOracleConfig::default(),
                state_cache.clone()
            ),
            RPC_DEFAULT_GAS_CAP,
            TracingCallPool::build().unwrap()
        );

        let tracing_call_guard = TracingCallGuard::new(10);

        let trace = TraceApi::new(provider, api.clone(), tracing_call_guard);

        Self { api, trace }
    }
}

/// re-implementation of 'view()'
/// allows for a function to be passed in through a RO libmdbx transaction
/// /reth/crates/storage/db/src/abstraction/database.rs
pub fn view<F, T>(db: &Env<WriteMap>, f: F) -> Result<T, DatabaseError>
where
    F: FnOnce(&<Env<WriteMap> as DatabaseGAT<'_>>::TX) -> T
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
        None
    )?;

    view(&db, |tx| {
        for table in tables::Tables::ALL.iter().map(|table| table.name()) {
            tx.inner
                .open_db(Some(table))
                .wrap_err("Could not open db.")
                .unwrap();
        }
    })?;

    Ok(db)
}
