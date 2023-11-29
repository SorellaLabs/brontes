use std::{collections::HashSet, fmt::Debug, path::Path, sync::Arc};

use eyre::Context;
use reth_beacon_consensus::BeaconConsensus;
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
use reth_network_api::noop::NoopNetwork;
use reth_primitives::{BlockId, PruneModes, ResultAndState, MAINNET};
use reth_provider::{
    providers::BlockchainProvider, ProviderFactory, StateProviderBox, TransactionsProvider,
};
use reth_revm::{
    database::{StateProviderDatabase, SubState},
    db::CacheDB,
    env::tx_env_with_recovered,
    tracing::{TracingInspector, TracingInspectorConfig},
    DatabaseCommit, Factory,
};
use reth_rpc::{
    eth::{
        cache::{EthStateCache, EthStateCacheConfig},
        error::EthResult,
        gas_oracle::{GasPriceOracle, GasPriceOracleConfig},
        EthTransactions, RPC_DEFAULT_GAS_CAP,
    },
    EthApi, TraceApi, TracingCallGuard, TracingCallPool,
};
use reth_rpc_types::{
    trace::parity::{TraceResultsWithTransactionHash, TraceType},
    BlockError, TransactionInfo,
};
use reth_tasks::TaskManager;
use reth_transaction_pool::{
    blobstore::NoopBlobStore, validate::EthTransactionValidatorBuilder, CoinbaseTipOrdering,
    EthPooledTransaction, EthTransactionValidator, Pool, TransactionValidationTaskExecutor,
};
use revm_primitives::ExecutionResult;
use tokio::runtime::Handle;

pub type Provider = BlockchainProvider<
    Arc<Env<WriteMap>>,
    ShareableBlockchainTree<Arc<Env<WriteMap>>, Arc<BeaconConsensus>, Factory>,
>;

pub type RethApi = EthApi<Provider, RethTxPool, NoopNetwork>;

pub type RethTxPool = Pool<
    TransactionValidationTaskExecutor<EthTransactionValidator<Provider, EthPooledTransaction>>,
    CoinbaseTipOrdering<EthPooledTransaction>,
    NoopBlobStore,
>;

pub struct TracingClient {
    pub api:   EthApi<Provider, RethTxPool, NoopNetwork>,
    pub trace: TraceApi<Provider, RethApi>,
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
            Arc::clone(&chain),
        );

        let tree_config = BlockchainTreeConfig::default();

        let (canon_state_notification_sender, _receiver) =
            tokio::sync::broadcast::channel(tree_config.max_reorg_depth() as usize * 2);

        let blockchain_tree = ShareableBlockchainTree::new(
            BlockchainTree::new(
                tree_externals,
                canon_state_notification_sender,
                tree_config,
                Some(PruneModes::none()),
            )
            .unwrap(),
        );

        let provider = BlockchainProvider::new(
            ProviderFactory::new(Arc::clone(&db), Arc::clone(&chain)),
            blockchain_tree,
        )
        .unwrap();

        let state_cache = EthStateCache::spawn(provider.clone(), EthStateCacheConfig::default());

        let transaction_validator = EthTransactionValidatorBuilder::new(chain.clone())
            .build_with_tasks(provider.clone(), task_executor.clone(), NoopBlobStore::default());

        let tx_pool = reth_transaction_pool::Pool::eth_pool(
            transaction_validator,
            NoopBlobStore::default(),
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
            RPC_DEFAULT_GAS_CAP,
            TracingCallPool::build().unwrap(),
        );

        let tracing_call_guard = TracingCallGuard::new(100);

        let trace = TraceApi::new(provider, api.clone(), tracing_call_guard);

        Self { api, trace }
    }

    /// Replays all transactions in a block
    pub async fn replay_block_transactions(
        &self,
        block_id: BlockId,
    ) -> EthResult<Option<Vec<TraceResultsWithTransactionHash>>> {
        let config = TracingInspectorConfig {
            record_logs:              true,
            record_steps:             false,
            record_state_diff:        false,
            record_stack_snapshots:   false,
            record_memory_snapshots:  false,
            record_call_return_data:  true,
            exclude_precompile_calls: false,
        };
        self.trace_block_with(
            block_id,
            config,
            |tx_info, tracing_inspector, execution_res, state, db| Ok(3),
        );
        todo!()
    }

    /// Executes all transactions of a block and returns a list of callback
    /// results invoked for each transaction in the block.
    ///
    /// This
    /// 1. fetches all transactions of the block
    /// 2. configures the EVM evn
    /// 3. loops over all transactions and executes them
    /// 4. calls the callback with the transaction info, the execution result,
    /// the changed state _after_ the transaction [StateProviderDatabase]
    /// and the database that points to the state right _before_ the
    /// transaction.
    async fn trace_block_with<F, R>(
        &self,
        block_id: BlockId,
        config: TracingInspectorConfig,
        f: F,
    ) -> EthResult<Option<Vec<R>>>
    where
        // This is the callback that's invoked for each transaction with
        F: for<'a> Fn(
                TransactionInfo,
                TracingInspector,
                ExecutionResult,
                &'a revm_primitives::State,
                &'a CacheDB<StateProviderDatabase<StateProviderBox<'a>>>,
            ) -> EthResult<R>
            + Send
            + 'static,
        R: Send + 'static,
    {
        let ((cfg, block_env, _), block) =
            futures::try_join!(self.api.evm_env_at(block_id), self.api.block_by_id(block_id),)?;

        let block = match block {
            Some(block) => block,
            None => return Ok(None),
        };

        // we need to get the state of the parent block because we're replaying this
        // block on top of its parent block's state
        let state_at = block.parent_hash;

        let block_hash = block.hash;
        let transactions = block.body;

        // replay all transactions of the block
        self.api
            .spawn_with_state_at_block(state_at.into(), move |state| {
                let mut results = Vec::with_capacity(transactions.len());
                let mut db = SubState::new(StateProviderDatabase::new(state));

                let mut transactions = transactions.into_iter().enumerate().peekable();

                while let Some((idx, tx)) = transactions.next() {
                    let tx = tx.into_ecrecovered().ok_or(BlockError::InvalidSignature)?;
                    let tx_info = TransactionInfo {
                        hash:         Some(tx.hash()),
                        index:        Some(idx as u64),
                        block_hash:   Some(block_hash),
                        block_number: Some(block_env.number.try_into().unwrap_or(u64::MAX)),
                        base_fee:     Some(block_env.basefee.try_into().unwrap_or(u64::MAX)),
                    };

                    let tx = tx_env_with_recovered(&tx);
                    let env =
                        revm_primitives::Env { cfg: cfg.clone(), block: block_env.clone(), tx };

                    let mut inspector = TracingInspector::new(config);
                    let (res, _) = inspect(&mut db, env, &mut inspector)?;
                    let ResultAndState { result, state } = res;
                    results.push(f(tx_info, inspector, result, &state, &db)?);

                    // need to apply the state changes of this transaction before executing the
                    // next transaction
                    if transactions.peek().is_some() {
                        // need to apply the state changes of this transaction before executing
                        // the next transaction
                        db.commit(state)
                    }
                }

                Ok(results)
            })
            .await
            .map(Some)
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
            tx.inner
                .open_db(Some(table))
                .wrap_err("Could not open db.")
                .unwrap();
        }
    })?;

    Ok(db)
}
