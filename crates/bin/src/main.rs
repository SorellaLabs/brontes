use std::{env, error::Error, time::Duration};

use tracing_subscriber::Layer;

#[cfg(all(feature = "jemalloc", unix))]
#[global_allocator]
static ALLOC: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

#[cfg(all(feature = "dhat-heap", not(feature = "jemalloc")))]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

use brontes::{
    cli::{Args, Commands},
    runner,
};
use clap::Parser;
use eyre::eyre;
use tracing::{error, info};
use tracing_subscriber::filter::Directive;

fn main() -> eyre::Result<()> {
    dotenv::dotenv().expect("Failed to load .env file");
    fdlimit::raise_fd_limit().unwrap();
    #[cfg(all(feature = "dhat-heap", not(feature = "jemalloc")))]
    let _profiler = dhat::Profiler::new_heap();
    match run() {
        Ok(()) => {
            info!(target: "brontes", "successful shutdown");
            Ok(())
        }
        Err(e) => {
            error!("Error: {:?}", e);

            let mut source: Option<&dyn Error> = e.source();
            while let Some(err) = source {
                error!("Caused by: {:?}", err);
                source = err.source();
            }
            Err(eyre!("program exited via error"))
        }
    }
}

fn run() -> eyre::Result<()> {
    let opt = Args::parse();
    let brontes_db_endpoint = opt
        .brontes_db_path
        .unwrap_or(env::var("BRONTES_DB_PATH").expect("No BRONTES_DB_PATH in .env"));

    init_tracing(opt.verbosity.directive());

    match opt.command {
        Commands::Run(command) => {
            runner::run_command_until_exit(opt.metrics_port, Duration::from_secs(60), |ctx| {
                command.execute(brontes_db_endpoint, ctx)
            })
        }
        Commands::Database(command) => {
            runner::run_command_until_exit(opt.metrics_port, Duration::from_secs(5), |ctx| {
                command.execute(brontes_db_endpoint, ctx)
            })
        }
    }
}

fn init_tracing(verbosity: Directive) {
    let layers = vec![
        brontes_tracing::stdout(verbosity),
        brontes_metrics::error_layer::BrontesErrorMetrics::default().boxed(),
    ];

    brontes_tracing::init(layers);
}
