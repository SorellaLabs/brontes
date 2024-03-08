use std::{env, path::Path};

use alloy_primitives::Address;
#[cfg(not(feature = "local-reth"))]
use brontes_core::local_provider::LocalProvider;
#[cfg(feature = "local-clickhouse")]
use brontes_database::clickhouse::Clickhouse;
#[cfg(not(feature = "local-clickhouse"))]
use brontes_database::clickhouse::ClickhouseHttpClient;
#[cfg(all(feature = "local-clickhouse", not(feature = "local-no-inserts")))]
use brontes_database::clickhouse::ClickhouseMiddleware;
use brontes_database::libmdbx::LibmdbxReadWriter;
use brontes_inspect::{Inspector, Inspectors};
use brontes_types::{
    db::{cex::CexExchange, traits::LibmdbxReader},
    init_pricing_threadpool, init_tree_threadpool,
    mev::Bundle,
    BrontesTaskExecutor,
};
use itertools::Itertools;
#[cfg(feature = "local-reth")]
use reth_tracing_ext::TracingClient;
use strum::IntoEnumIterator;
use tracing::info;

#[cfg(any(not(feature = "local-clickhouse"), feature = "local-no-inserts"))]
pub fn load_database(db_endpoint: String) -> eyre::Result<LibmdbxReadWriter> {
    LibmdbxReadWriter::init_db(db_endpoint, None)
}

// This version is used when `local-clickhouse` is enabled but
// `local-no-inserts` is not.
#[cfg(all(feature = "local-clickhouse", not(feature = "local-no-inserts")))]
pub fn load_database(db_endpoint: String) -> eyre::Result<ClickhouseMiddleware<LibmdbxReadWriter>> {
    let inner = LibmdbxReadWriter::init_db(db_endpoint, None)?;
    let clickhouse = Clickhouse::default();
    Ok(ClickhouseMiddleware::new(clickhouse, inner))
}

#[cfg(feature = "local-clickhouse")]
pub async fn load_clickhouse() -> eyre::Result<Clickhouse> {
    Ok(Clickhouse::default())
}

#[cfg(not(feature = "local-clickhouse"))]
pub async fn load_clickhouse() -> eyre::Result<ClickhouseHttpClient> {
    let clickhouse_api = env::var("CLICKHOUSE_API")?;
    let clickhouse_api_key = env::var("CLICKHOUSE_API_KEY").ok();
    Ok(ClickhouseHttpClient::new(clickhouse_api, clickhouse_api_key).await)
}

#[cfg(not(feature = "local-reth"))]
pub fn get_tracing_provider(_: &Path, _: u64, _: BrontesTaskExecutor) -> LocalProvider {
    let db_endpoint = env::var("RETH_ENDPOINT").expect("No db Endpoint in .env");
    let db_port = env::var("RETH_PORT").expect("No DB port.env");
    let url = format!("{db_endpoint}:{db_port}");
    LocalProvider::new(url, 5)
}

#[cfg(feature = "local-reth")]
pub fn get_tracing_provider(
    db_path: &Path,
    tracing_tasks: u64,
    executor: BrontesTaskExecutor,
) -> TracingClient {
    TracingClient::new(db_path, tracing_tasks, executor.clone())
}

/// initalizes the global tree threadpool and pricing threadpool given
/// the amount of threads passed in.
/// the pricing threadpool is allocated 65% while the tree is allocated 35%
pub fn init_thread_pools(max_tasks: u64) {
    let tree_tasks = (max_tasks as f64 * 0.35) as usize + 1;
    let pricing_tasks = (max_tasks as f64 * 0.65) as usize + 1;
    init_pricing_threadpool(pricing_tasks);
    init_tree_threadpool(tree_tasks);
}

pub fn determine_max_tasks(max_tasks: Option<u64>) -> u64 {
    match max_tasks {
        Some(max_tasks) => max_tasks,
        None => {
            let cpus = num_cpus::get_physical();
            (cpus as f64 * 0.30) as u64 // 30% of physical cores
        }
    }
}

pub fn static_object<T>(obj: T) -> &'static T {
    &*Box::leak(Box::new(obj))
}

pub fn init_inspectors<DB: LibmdbxReader>(
    quote_token: Address,
    db: &'static DB,
    inspectors: Option<Vec<Inspectors>>,
    cex_exchanges: Option<Vec<String>>,
) -> &'static [&'static dyn Inspector<Result = Vec<Bundle>>] {
    let cex_exchanges: Vec<CexExchange> = cex_exchanges
        .unwrap_or_default()
        .into_iter()
        .map(|s| s.into())
        .collect();

    let mut res = Vec::new();
    for inspector in inspectors
        .map(|i| i.into_iter())
        .unwrap_or_else(|| Inspectors::iter().collect_vec().into_iter())
    {
        res.push(inspector.init_mev_inspector(quote_token, db, &cex_exchanges));
    }

    &*Box::leak(res.into_boxed_slice())
}

pub fn get_env_vars() -> eyre::Result<String> {
    let db_path = env::var("DB_PATH").map_err(|_| Box::new(std::env::VarError::NotPresent))?;
    info!("Found DB Path");

    Ok(db_path)
}
