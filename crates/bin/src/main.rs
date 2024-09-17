use std::{env, error::Error, time::Duration};

use brontes_tracing::BoxedLayer;
use tracing_subscriber::{Layer, Registry};
use log_report_layer::TelegramConfig;
use tracing::Level;

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
    if dotenv::dotenv().is_err() {
        eprintln!("Failed to load .env file");
    };

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
    let brontes_db_path = opt
        .brontes_db_path
        .unwrap_or(env::var("BRONTES_DB_PATH").expect("No BRONTES_DB_PATH in .env"));

    init_tracing(opt.verbosity.directive());

    let metrics_port = if opt.skip_prometheus { None } else { Some(opt.metrics_port) };

    match opt.command {
        Commands::Run(command) => {
            runner::run_command_until_exit(metrics_port, Duration::from_secs(3600), |ctx| {
                command.execute(brontes_db_path, ctx)
            })
        }
        Commands::Database(command) => {
            runner::run_command_until_exit(None, Duration::from_secs(5), |ctx| {
                command.execute(brontes_db_path, ctx)
            })
        }
    }
}

fn init_tracing(verbosity: Directive) {
    let layers = if cfg!(feature = "sorella-server") {
        vec![
            brontes_tracing::stdout(verbosity),
            brontes_metrics::error_layer::BrontesErrorMetrics::default().boxed(),
            initialize_telegram_error_layer(),
        ]
    } else {
        vec![
            brontes_tracing::stdout(verbosity),
            brontes_metrics::error_layer::BrontesErrorMetrics::default().boxed(),
        ]
    };

    brontes_tracing::init(layers);
}

fn initialize_telegram_error_layer() -> BoxedLayer<Registry> {
    // build
    let bot_token = std::env::var("BOT_ID").unwrap();
    let chat_id = std::env::var("CHAT_ID").unwrap();
    let tag_users = std::env::var("USERS_TO_TAG")
        .unwrap()
        .split(',')
        .map(|s| s.to_string())
        .collect::<Vec<String>>();

    let client = reqwest::blocking::Client::new();
    let telegram =
        TelegramConfig::new("Brontes".to_string(), tag_users, bot_token, chat_id, client)
            .build_layer(vec![Level::ERROR]);

    telegram.boxed()
}
