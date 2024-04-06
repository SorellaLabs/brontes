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
#[cfg(all(feature = "local-clickhouse", not(feature = "local-no-inserts")))]
use brontes_database::clickhouse::ReadOnlyMiddleware;
use brontes_database::{clickhouse::cex_config::CexDownloadConfig, libmdbx::LibmdbxReadWriter};
use brontes_inspect::{Inspector, Inspectors};
use brontes_metrics::{PoirotMetricEvents, PoirotMetricsListener};
use brontes_types::{
    constants::{USDC_ADDRESS, USDT_ADDRESS},
    db::{cex::CexExchange, traits::LibmdbxReader},
    mev::Bundle,
    BrontesTaskExecutor,
};
use itertools::Itertools;
#[cfg(feature = "local-reth")]
use reth_tracing_ext::TracingClient;
use strum::IntoEnumIterator;
use tokio::sync::mpsc::{unbounded_channel, UnboundedSender};
use tracing::info;

use super::run::RunArgs;

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

#[cfg(all(feature = "local-clickhouse", not(feature = "local-no-inserts")))]
pub fn load_read_only_database(
    db_endpoint: String,
) -> eyre::Result<ReadOnlyMiddleware<LibmdbxReadWriter>> {
    let inner = LibmdbxReadWriter::init_db(db_endpoint, None)?;
    let clickhouse = Clickhouse::default();
    Ok(ReadOnlyMiddleware::new(clickhouse, inner))
}

pub fn load_libmdbx(db_endpoint: String) -> eyre::Result<LibmdbxReadWriter> {
    LibmdbxReadWriter::init_db(db_endpoint, None)
}

#[cfg(feature = "local-clickhouse")]
pub async fn load_clickhouse(cex_download_config: CexDownloadConfig) -> eyre::Result<Clickhouse> {
    Ok(Clickhouse::new(Default::default(), cex_download_config))
}

#[cfg(not(feature = "local-clickhouse"))]
pub async fn load_clickhouse(
    cex_download_config: CexDownloadConfig,
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
) -> &'static [&'static dyn Inspector<Result = Vec<Bundle>>] {
    let mut res = Vec::new();
    for inspector in inspectors
        .map(|i| i.into_iter())
        .unwrap_or_else(|| Inspectors::iter().collect_vec().into_iter())
    {
        res.push(inspector.init_mev_inspector(quote_token, db, &cex_exchanges));
    }

    &*Box::leak(res.into_boxed_slice())
}

pub fn init_brontes_db() -> eyre::Result<&'static LibmdbxReadWriter> {
    let brontes_db_path = env::var("BRONTES_DB_PATH").expect("No BRONTES_DB_PATH in .env");
    let libmdbx = static_object(load_database(brontes_db_path)?);
    tracing::info!(target: "brontes", "Brontes database initialized");
    Ok(libmdbx)
}

pub fn init_tracer(
    task_executor: BrontesTaskExecutor,
    max_tasks: u64,
) -> eyre::Result<&'static LocalProvider> {
    let reth_db_path = env::var("BRONTES_DB_PATH").expect("No BRONTES_DB_PATH in .env");
    let tracer =
        static_object(get_tracing_provider(Path::new(&reth_db_path), max_tasks, task_executor));
    tracing::info!(target: "brontes", "Tracer initialized");
    Ok(tracer)
}

pub fn init_metrics_listener(
    task_executor: &BrontesTaskExecutor,
) -> UnboundedSender<PoirotMetricEvents> {
    let (metrics_tx, metrics_rx) = unbounded_channel();
    let metrics_listener = PoirotMetricsListener::new(metrics_rx);
    task_executor.spawn_critical("metrics", metrics_listener);
    metrics_tx
}

pub async fn init_clickhouse(run_args: &RunArgs) -> eyre::Result<&'static ClickhouseHttpClient> {
    let cex_download_config = CexDownloadConfig::new(
        (run_args.cex_time_window_before, run_args.cex_time_window_after),
        run_args.cex_exchanges.clone(),
    );

    let clickhouse = static_object(load_clickhouse(cex_download_config).await?);
    tracing::info!(target: "brontes", "Initialized Clickhouse connection");
    Ok(clickhouse)
}

pub fn load_quote_address(run_args: &RunArgs) -> eyre::Result<Address> {
    let quote_asset = run_args.quote_asset.parse()?;

    match quote_asset {
        USDC_ADDRESS => tracing::info!(target: "brontes", "Set USDC as quote asset"),
        USDT_ADDRESS => tracing::info!(target: "brontes", "Set USDT as quote asset"),
        _ => tracing::info!(target: "brontes", "Set quote asset"),
    }

    Ok(quote_asset)
}

pub fn set_dex_pricing(run_args: &mut RunArgs) {
    let only_cex_dex = run_args
        .inspectors
        .as_ref()
        .map(|f| {
            #[cfg(not(feature = "cex-dex-markout"))]
            let cmp = Inspectors::CexDex;
            #[cfg(feature = "cex-dex-markout")]
            let cmp = Inspectors::CexDexMarkout;
            f.len() == 1 && f.contains(&cmp)
        })
        .unwrap_or(false);

    if only_cex_dex {
        run_args.force_no_dex_pricing = true;
    }
}

pub fn get_db_path() -> eyre::Result<String> {
    let db_path = env::var("DB_PATH").expect("DB path is not present in env");
    info!("Found DB Path");

    Ok(db_path)
}
