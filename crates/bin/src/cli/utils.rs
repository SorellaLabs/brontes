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
#[cfg(all(feature = "local-clickhouse", not(feature = "local-no-inserts")))]
use brontes_database::clickhouse::{dbms::BrontesClickhouseTableDataTypes, ClickhouseBuffered};
use brontes_database::libmdbx::LibmdbxReadWriter;
use brontes_inspect::{Inspector, Inspectors};
use brontes_types::{
    db::{cex::CexExchange, traits::LibmdbxReader},
    mev::Bundle,
    BrontesTaskExecutor,
};
#[cfg(all(feature = "local-clickhouse", not(feature = "local-no-inserts")))]
use futures::StreamExt;
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
pub async fn load_clickhouse(
    _cex_download_config: brontes_database::clickhouse::cex_config::CexDownloadConfig,
) -> eyre::Result<Clickhouse> {
    #[cfg(all(feature = "local-clickhouse", not(feature = "local-no-inserts")))]
    {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        spawn_db_writer_thread(rx);
        Ok(Clickhouse::new(Default::default(), _cex_download_config, Some(tx)))
    }

    #[cfg(all(feature = "local-clickhouse", feature = "local-no-inserts"))]
    {
        Ok(Clickhouse::default())
    }
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

pub fn get_env_vars() -> eyre::Result<String> {
    let db_path = env::var("DB_PATH").map_err(|_| Box::new(std::env::VarError::NotPresent))?;
    info!("Found DB Path");

    Ok(db_path)
}

#[cfg(all(feature = "local-clickhouse", not(feature = "local-no-inserts")))]
fn spawn_db_writer_thread(
    buffered_rx: tokio::sync::mpsc::UnboundedReceiver<Vec<BrontesClickhouseTableDataTypes>>,
) {
    let handle = tokio::runtime::Handle::current();
    std::thread::spawn(move || {
        handle.block_on(async move {
            let mut clickhouse_writer = ClickhouseBuffered::new(buffered_rx, 10);
            while let Some(val) = clickhouse_writer.next().await {
                if let Err(e) = val {
                    tracing::error!(target: "brontes", "error writing to clickhouse {:?}", e);
                }
            }
        })
    });
}
