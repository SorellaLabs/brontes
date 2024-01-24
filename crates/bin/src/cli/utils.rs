use std::{env, path::Path};

use alloy_primitives::Address;
#[cfg(feature = "local")]
use brontes_core::local_provider::LocalProvider;
use brontes_database::libmdbx::LibmdbxReadWriter;
use brontes_inspect::{Inspector, Inspectors};
use itertools::Itertools;
use reth_tasks::TaskExecutor;
use reth_tracing_ext::TracingClient;
use strum::IntoEnumIterator;
use tracing::info;

pub fn determine_max_tasks(max_tasks: Option<u64>) -> u64 {
    match max_tasks {
        Some(max_tasks) => max_tasks as u64,
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
) -> &'static [&'static Box<dyn Inspector>] {
    let mut res = Vec::new();
    for inspector in inspectors
        .map(|i| i.into_iter())
        .unwrap_or_else(|| Inspectors::iter().collect_vec().into_iter())
    {
        res.push(inspector.init_inspector(quote_token, db));
    }

    &*Box::leak(res.into_boxed_slice())
}

pub fn get_env_vars() -> eyre::Result<String> {
    let db_path = env::var("DB_PATH").map_err(|_| Box::new(std::env::VarError::NotPresent))?;
    info!("Found DB Path");

    Ok(db_path)
}

#[cfg(feature = "local")]
pub fn get_tracing_provider(_: Path, _: u64, _: TaskExecutor) -> LocalProvider {
    let db_endpoint = env::var("RETH_ENDPOINT").expect("No db Endpoint in .env");
    let db_port = env::var("RETH_PORT").expect("No DB port.env");
    let url = format!("{db_endpoint}:{db_port}");
    LocalProvider::new(url)
}

#[cfg(not(feature = "local"))]
pub fn get_tracing_provider(
    db_path: &Path,
    tracing_tasks: u64,
    executor: TaskExecutor,
) -> TracingClient {
    TracingClient::new(db_path, tracing_tasks, executor.clone())
}
