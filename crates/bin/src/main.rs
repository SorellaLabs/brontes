use std::{
    env,
    error::Error,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use alloy_primitives::Address;
use async_scoped::{Scope, TokioScope};
use banner::print_banner;
use brontes::{Brontes, DataBatching, PROMETHEUS_ENDPOINT_IP, PROMETHEUS_ENDPOINT_PORT};
use brontes_classifier::Classifier;
use brontes_core::decoding::Parser as DParser;
use brontes_database::{
    clickhouse::Clickhouse,
    libmdbx::{
        cursor::CompressedCursor,
        tables::{AddressToProtocol, CompressedTable, IntoTableKey, Tables},
        Libmdbx,
    },
};
use brontes_inspect::{
    atomic_backrun::AtomicBackrunInspector, cex_dex::CexDexInspector, jit::JitInspector,
    sandwich::SandwichInspector, Inspector,
};
use brontes_metrics::{prometheus_exporter::initialize, PoirotMetricsListener};
use clap::Parser;
use itertools::Itertools;
use metrics_process::Collector;
use reth_db::mdbx::RO;
use reth_tracing_ext::TracingClient;
use tokio::sync::mpsc::unbounded_channel;
use tracing::{error, info, Level};
use tracing_subscriber::filter::Directive;

mod banner;
mod cli;

#[cfg(feature = "tests")]
use cli::TraceArg;
use cli::{Args, Commands};

fn main() {
    print_banner();
    dotenv::dotenv().ok();

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    let brontes_directive: Directive = format!("brontes={}", Level::INFO).parse().unwrap();
    let tracing_directive: Directive = format!("reth-tracing-ext={}", Level::INFO).parse().unwrap();

    let layers = vec![
        brontes_tracing::stdout(tracing_directive),
        brontes_tracing::stdout(brontes_directive),
    ];

    //let subscriber =
    // Registry::default().with(tracing_subscriber::fmt::layer().
    // with_filter(filter));

    //tracing::subscriber::set_global_default(subscriber)
    //  .expect("Could not set global default subscriber");
    brontes_tracing::init(layers);

    match runtime.block_on(run()) {
        Ok(()) => info!(target: "brontes", "SUCCESS!"),
        Err(e) => {
            error!("Error: {:?}", e);

            let mut source: Option<&dyn Error> = e.source();
            while let Some(err) = source {
                error!("Caused by: {:?}", err);
                source = err.source();
            }
        }
    }
}

async fn run() -> Result<(), Box<dyn Error>> {
    // initalize_prometheus().await;
    // parse cli
    let opt = Args::parse();

    match opt.command {
        Commands::Run(command) => command.execute().await,
        Commands::Init(command) => command.execute().await,
        Commands::RunBatchWithPricing(command) => command.execute().await,
        Commands::QueryDb(command) => command.execute().await,
        Commands::AddToDb(command) => command.execute().await,
        #[cfg(feature = "tests")]
        Commands::Traces(args) => command.execute().await,
    }
}

async fn initialize_prometheus() {
    // initializes the prometheus endpoint
    initialize(
        SocketAddr::new(
            IpAddr::V4(Ipv4Addr::from(PROMETHEUS_ENDPOINT_IP)),
            PROMETHEUS_ENDPOINT_PORT,
        ),
        Collector::default(),
    )
    .await
    .unwrap();
    info!("Initialized prometheus endpoint");
}
