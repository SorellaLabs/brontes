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
            (cpus as f64 * 0.30) as u64 // 30% of physical cores
        }
    }
}

pub fn static_object<T>(obj: T) -> &'static T {
    &*Box::leak(Box::new(obj))
}

pub fn init_all_inspectors(
    quote_token: Address,
    db: &'static LibmdbxReadWriter,
) -> &'static [&'static Box<dyn Inspector>] {
    let sandwich = static_object(
        Box::new(SandwichInspector::new(quote_token, db)) as Box<dyn Inspector + 'static>
    );
    let cex_dex = static_object(
        Box::new(CexDexInspector::new(quote_token, db)) as Box<dyn Inspector + 'static>
    );
    let jit =
        static_object(Box::new(JitInspector::new(quote_token, db)) as Box<dyn Inspector + 'static>);

    let backrun = static_object(
        Box::new(AtomicBackrunInspector::new(quote_token, db)) as Box<dyn Inspector + 'static>
    );

    static_object(vec![sandwich, cex_dex, jit, backrun].into_boxed_slice())
}

pub fn get_env_vars() -> eyre::Result<String> {
    let db_path = env::var("DB_PATH").map_err(|_| Box::new(std::env::VarError::NotPresent))?;
    info!("Found DB Path");

    Ok(db_path)
}
