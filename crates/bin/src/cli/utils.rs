use std::env;

use alloy_primitives::Address;
use brontes_database::libmdbx::{Libmdbx, LibmdbxReadWriter};
use brontes_inspect::{
    atomic_backrun::AtomicBackrunInspector, cex_dex::CexDexInspector, jit::JitInspector,
    sandwich::SandwichInspector, Inspector,
};
use tracing::info;

pub fn determine_max_tasks(max_tasks: Option<u64>) -> u64 {
    match max_tasks {
        Some(max_tasks) => max_tasks as u64,
        None => {
            let cpus = num_cpus::get_physical();
            (cpus as f64 * 0.25) as u64 // 25% of physical cores
        }
    }
}

pub fn init_all_inspectors<'a>(
    quote_token: Address,
    db: &'static LibmdbxReadWriter,
) -> &'static [&'static Box<dyn Inspector>] {
    let sandwich = &*Box::leak(Box::new(
        Box::new(SandwichInspector::new(quote_token, db)) as Box<dyn Inspector + 'static>
    ));
    let cex_dex = &*Box::leak(Box::new(
        Box::new(CexDexInspector::new(quote_token, db)) as Box<dyn Inspector + 'static>
    ));
    let jit = &*Box::leak(Box::new(
        Box::new(JitInspector::new(quote_token, db)) as Box<dyn Inspector + 'static>
    ));
    let backrun = &*Box::leak(Box::new(
        Box::new(AtomicBackrunInspector::new(quote_token, db)) as Box<dyn Inspector + 'static>
    ));

    &*Box::leak(vec![sandwich, cex_dex, jit, backrun].into_boxed_slice())
}

pub fn get_env_vars() -> eyre::Result<String> {
    let db_path = env::var("DB_PATH").map_err(|_| Box::new(std::env::VarError::NotPresent))?;
    info!("Found DB Path");

    Ok(db_path)
}
