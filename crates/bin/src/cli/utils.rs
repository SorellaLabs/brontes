use std::{env, path::Path, sync::Arc};

use alloy_primitives::Address;
#[cfg(feature = "local")]
use brontes_core::local_provider::LocalProvider;
use brontes_database::libmdbx::LibmdbxReadWriter;
use brontes_inspect::{Inspector, Inspectors};
use brontes_types::{db::cex::CexExchange, mev::Bundle};
use itertools::Itertools;
use reth_tasks::TaskExecutor;
#[cfg(not(feature = "local"))]
use reth_tracing_ext::TracingClient;
use strum::IntoEnumIterator;
use tracing::info;

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

pub fn init_inspectors(
    quote_token: Address,
    db: &'static LibmdbxReadWriter,
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

#[cfg(feature = "local")]
pub fn get_tracing_provider(_: &Path, _: u64, _: TaskExecutor) -> LocalProvider {
    let db_endpoint = env::var("RETH_ENDPOINT").expect("No db Endpoint in .env");
    let db_port = env::var("RETH_PORT").expect("No DB port.env");
    let db_path = env::var("DB_PATH").expect("No DB in .env");
    let db = reth_db::open_db(Path::new(&db_path), Default::default()).expect("Could not open db");
    let url = format!("{db_endpoint}:{db_port}");
    LocalProvider::new(url, Arc::new(db))
}

#[cfg(not(feature = "local"))]
pub fn get_tracing_provider(
    db_path: &Path,
    tracing_tasks: u64,
    executor: TaskExecutor,
) -> TracingClient {
    TracingClient::new(db_path, tracing_tasks, executor.clone())
}
