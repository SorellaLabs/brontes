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
#[cfg(not(target_env = "msvc"))]
use jemallocator::Jemalloc;
use tracing::{error, info, Level};
use tracing_subscriber::{filter::Directive, Layer};
use tui_logger::tracing_subscriber_layer;

#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;

fn main() -> eyre::Result<()> {
    dotenv::dotenv().ok();
    fdlimit::raise_fd_limit().unwrap();
    #[cfg(all(feature = "dhat-heap", not(feature = "jemalloc")))]
    let _profiler = dhat::Profiler::new_heap();
    match run() {
        Ok(()) => {
            info!(target: "brontes", "successful shutdown");
            Ok(())
        }
        Err(e) => {
            error!(target: "brontes", "ERROR: {:?}", e);

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
            init_tracing(command.cli_only);

            if command.cli_only {
                runner::run_command_until_exit(|ctx| command.execute(ctx))
            } else {
                runner::run_command_until_exit(|ctx| command.execute(ctx))
            }
        }
        Commands::Database(command) => runner::run_command_until_exit(|ctx| command.execute(ctx)),
        Commands::Analytics(command) => runner::run_command_until_exit(|ctx| command.execute(ctx)),
    }
}
fn init_tracing(tui: bool) {

    if !tui {
        let layers = vec![tracing_subscriber_layer().boxed()];
        brontes_tracing::init(layers);
    } else {
        let verbosity_level = Level::INFO;
        let directive: Directive = format!("{verbosity_level}").parse().unwrap();

        let layers = vec![brontes_tracing::stdout(directive)];

        brontes_tracing::init(layers);
    }
}