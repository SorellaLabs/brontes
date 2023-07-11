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
    pub filter: EthFilter<Provider, RethTxPool>,
}