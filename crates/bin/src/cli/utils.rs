use std::{env, path::Path};

use alloy_primitives::Address;
#[cfg(not(feature = "local-reth"))]
use brontes_core::local_provider::LocalProvider;
#[cfg(all(feature = "local-clickhouse", not(feature = "local-no-inserts")))]
use brontes_database::clickhouse::clickhouse_config;
#[cfg(feature = "local-clickhouse")]
use brontes_database::clickhouse::Clickhouse;
#[cfg(not(feature = "local-clickhouse"))]
use brontes_database::clickhouse::ClickhouseHttpClient;
#[cfg(all(feature = "local-clickhouse", feature = "local-no-inserts"))]
use brontes_database::clickhouse::ClickhouseMiddleware;
#[cfg(all(feature = "local-clickhouse", not(feature = "local-no-inserts")))]
use brontes_database::clickhouse::ClickhouseMiddleware;
#[cfg(all(feature = "local-clickhouse", not(feature = "local-no-inserts")))]
use brontes_database::clickhouse::ReadOnlyMiddleware;
#[cfg(all(feature = "local-clickhouse", not(feature = "local-no-inserts")))]
use brontes_database::clickhouse::{dbms::BrontesClickhouseData, ClickhouseBuffered};
use brontes_database::libmdbx::LibmdbxReadWriter;
use brontes_inspect::{Inspector, Inspectors};
use brontes_metrics::inspectors::OutlierMetrics;
#[cfg(all(feature = "local-clickhouse", not(feature = "local-no-inserts")))]
use brontes_types::UnboundedYapperReceiver;
use brontes_types::{
    db::{cex::CexExchange, traits::LibmdbxReader},
    mev::Bundle,
    BrontesTaskExecutor,
};
use itertools::Itertools;
#[cfg(feature = "local-reth")]
use reth_tracing_ext::TracingClient;
use strum::IntoEnumIterator;
use tracing::info;

#[cfg(not(any(feature = "local-clickhouse", feature = "local-no-inserts")))]
pub fn load_database(
    executor: &BrontesTaskExecutor,
    db_endpoint: String,
) -> eyre::Result<LibmdbxReadWriter> {
    LibmdbxReadWriter::init_db(db_endpoint, None, executor)
}

#[cfg(not(any(feature = "local-clickhouse", feature = "local-no-inserts")))]
pub fn load_tip_database(cur: &LibmdbxReadWriter) -> eyre::Result<LibmdbxReadWriter> {
    Ok(cur.clone())
}

/// This version is used when `local-clickhouse` is enabled but
/// `local-no-inserts` is not.
#[cfg(all(feature = "local-clickhouse", feature = "local-no-inserts"))]
pub fn load_database(
    executor: &BrontesTaskExecutor,
    db_endpoint: String,
) -> eyre::Result<ClickhouseMiddleware<LibmdbxReadWriter>> {
    let inner = LibmdbxReadWriter::init_db(db_endpoint, None, executor)?;
    let clickhouse = Clickhouse::default();
    Ok(ClickhouseMiddleware::new(clickhouse, inner.into()))
}

/// This version is used when `local-clickhouse` is enabled but
/// `local-no-inserts` is not. for tip tracer
#[cfg(all(feature = "local-clickhouse", feature = "local-no-inserts"))]
pub fn load_tip_database(
    cur: &ClickhouseMiddleware<LibmdbxReadWriter>,
) -> eyre::Result<ClickhouseMiddleware<LibmdbxReadWriter>> {
    let mut tip = cur.clone();
    tip.client.tip = true;
    Ok(tip)
}

/// This version is used when `local-clickhouse` and
/// `local-no-inserts` is enabled
#[cfg(all(feature = "local-clickhouse", not(feature = "local-no-inserts")))]
pub fn load_database(
    executor: &BrontesTaskExecutor,
    db_endpoint: String,
) -> eyre::Result<ClickhouseMiddleware<LibmdbxReadWriter>> {
    let inner = LibmdbxReadWriter::init_db(db_endpoint, None, executor)?;

    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    spawn_db_writer_thread(executor, rx);
    let mut clickhouse = Clickhouse::default();
    clickhouse.buffered_insert_tx = Some(tx);

    Ok(ClickhouseMiddleware::new(clickhouse, inner.into()))
}

/// This version is used when `local-clickhouse` and
/// `local-no-inserts` is enabled this also will set a config in the clickhouse
/// to ensure that
#[cfg(all(feature = "local-clickhouse", not(feature = "local-no-inserts")))]
pub fn load_tip_database(
    cur: &ClickhouseMiddleware<LibmdbxReadWriter>,
) -> eyre::Result<ClickhouseMiddleware<LibmdbxReadWriter>> {
    let mut tip = cur.clone();
    tip.client.tip = true;
    Ok(tip)
}

#[cfg(all(feature = "local-clickhouse", not(feature = "local-no-inserts")))]
pub fn load_read_only_database(
    executor: &BrontesTaskExecutor,
    db_endpoint: String,
) -> eyre::Result<ReadOnlyMiddleware<LibmdbxReadWriter>> {
    let inner = LibmdbxReadWriter::init_db(db_endpoint, None, executor)?;
    let clickhouse = Clickhouse::default();
    Ok(ReadOnlyMiddleware::new(clickhouse, inner.into()))
}

pub fn load_libmdbx(
    executor: &BrontesTaskExecutor,
    db_endpoint: String,
) -> eyre::Result<LibmdbxReadWriter> {
    LibmdbxReadWriter::init_db(db_endpoint, None, executor)
}

#[allow(clippy::field_reassign_with_default)]
#[cfg(feature = "local-clickhouse")]
pub async fn load_clickhouse(
    cex_download_config: brontes_database::clickhouse::cex_config::CexDownloadConfig,
) -> eyre::Result<Clickhouse> {
    let mut clickhouse = Clickhouse::default();
    clickhouse.cex_download_config = cex_download_config;
    Ok(clickhouse)
}

#[cfg(not(feature = "local-clickhouse"))]
pub async fn load_clickhouse(
    cex_download_config: brontes_database::clickhouse::cex_config::CexDownloadConfig,
) -> eyre::Result<ClickhouseHttpClient> {
    let clickhouse_api = env::var("CLICKHOUSE_API")?;
    let clickhouse_api_key = env::var("CLICKHOUSE_API_KEY").ok();
    Ok(ClickhouseHttpClient::new(clickhouse_api, clickhouse_api_key, cex_download_config).await)
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
    cex_exchanges: Vec<CexExchange>,
    metrics: bool,
) -> &'static [&'static dyn Inspector<Result = Vec<Bundle>>] {
    let mut res = Vec::new();
    let metrics = metrics.then(OutlierMetrics::new);
    for inspector in inspectors
        .map(|i| i.into_iter())
        .unwrap_or_else(|| Inspectors::iter().collect_vec().into_iter())
    {
        res.push(inspector.init_mev_inspector(quote_token, db, &cex_exchanges, metrics.clone()));
    }

    &*Box::leak(res.into_boxed_slice())
}

pub fn get_env_vars() -> eyre::Result<String> {
    let db_path = env::var("DB_PATH").map_err(|_| Box::new(std::env::VarError::NotPresent))?;
    info!("Found DB Path");

    Ok(db_path)
}

#[cfg(all(feature = "local-clickhouse", not(feature = "local-no-inserts")))]
fn spawn_db_writer_thread(
    executor: &BrontesTaskExecutor,
    buffered_rx: tokio::sync::mpsc::UnboundedReceiver<Vec<BrontesClickhouseData>>,
) {
    let shutdown = executor.get_graceful_shutdown();
    ClickhouseBuffered::new(
        UnboundedYapperReceiver::new(buffered_rx, 1500, "clickhouse buffered".to_string()),
        clickhouse_config(),
        5000,
        800,
    )
    .run(shutdown);
    tracing::info!("started writer");
}
