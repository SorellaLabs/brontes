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

use banner::print_banner;
#[cfg(feature = "tests")]
use cli::TraceArg;
use cli::{Args, Commands};

type Inspectors<'a> = &'a [&'a Box<dyn Inspector>];

struct InspectorHolder {
    sandwich: Box<dyn Inspector>,
    cex_dex:  Box<dyn Inspector>,
    jit:      Box<dyn Inspector>,
    backrun:  Box<dyn Inspector>,
}

impl InspectorHolder {
    fn new(quote_token: Address, db: &'static Libmdbx) -> Self {
        Self {
            sandwich: Box::new(SandwichInspector::new(quote_token, db)),
            cex_dex:  Box::new(CexDexInspector::new(quote_token, db)),
            jit:      Box::new(JitInspector::new(quote_token, db)),
            backrun:  Box::new(AtomicBackrunInspector::new(quote_token, db)),
        }
    }

    fn get_inspectors(&'static self) -> Inspectors<'static> {
        &*Box::leak(Box::new([&self.sandwich, &self.cex_dex, &self.jit, &self.backrun]))
    }
}

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

async fn run_brontes(run_config: RunArgs) -> Result<(), Box<dyn Error>> {
    initialize_prometheus().await;

    // Fetch required environment variables.
    let db_path = get_env_vars()?;

    let max_tasks = determine_max_tasks(run_config.max_tasks);

    let (metrics_tx, metrics_rx) = unbounded_channel();

    let metrics_listener = PoirotMetricsListener::new(metrics_rx);
    tokio::spawn(metrics_listener);

    let brontes_db_endpoint = env::var("BRONTES_DB_PATH").expect("No BRONTES_DB_PATH in .env");
    let libmdbx =
        Box::leak(Box::new(Libmdbx::init_db(brontes_db_endpoint, None)?)) as &'static Libmdbx;
    let clickhouse = Clickhouse::default();

    let inspector_holder = Box::leak(Box::new(InspectorHolder::new(
        run_config.quote_asset.parse().unwrap(),
        &libmdbx,
    )));

    let inspectors: Inspectors = inspector_holder.get_inspectors();

    let (manager, tracer) =
        TracingClient::new(Path::new(&db_path), tokio::runtime::Handle::current(), max_tasks);
    tokio::spawn(manager);

    let parser = DParser::new(
        metrics_tx,
        &libmdbx,
        tracer.clone(),
        Box::new(|address, db_tx| db_tx.get::<AddressToProtocol>(*address).unwrap().is_none()),
    );

    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    let classifier = Classifier::new(&libmdbx, tx.clone(), tracer.into());

    #[cfg(not(feature = "local"))]
    let chain_tip = parser.get_latest_block_number().unwrap();
    #[cfg(feature = "local")]
    let chain_tip = parser.get_latest_block_number().await.unwrap();

    let brontes = Brontes::new(
        run_config.start_block,
        run_config.end_block,
        chain_tip,
        max_tasks.into(),
        &parser,
        &clickhouse,
        &libmdbx,
        &classifier,
        &inspectors,
    );
    brontes.await;
    info!("finnished running brontes, shutting down");
    std::thread::spawn(move || {
        drop(parser);
    });

    Ok(())
}

fn determine_max_tasks(max_tasks: Option<u64>) -> u64 {
    match max_tasks {
        Some(max_tasks) => max_tasks as u64,
        None => {
            let cpus = num_cpus::get_physical();
            (cpus as f64 * 0.5) as u64 // 50% of physical cores
        }
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

fn get_env_vars() -> Result<String, Box<dyn Error>> {
    let db_path = env::var("DB_PATH").map_err(|_| Box::new(std::env::VarError::NotPresent))?;
    info!("Found DB Path");

    Ok(db_path)
}

/*
fn get_reth_provider<T>() -> Result<Provider<T>, Box<dyn Error>> {
    let reth_url = env::var("RETH_ENDPOINT").expect("No RETH_DB Endpoint in .env");
    let reth_port = env::var("RETH_PORT").expect("No DB port.env");
    let url = format!("{reth_url}:{reth_port}");
    Provider::new(&url).unwrap()
}
 */
