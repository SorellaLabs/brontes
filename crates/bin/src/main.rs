use std::{
    env,
    error::Error,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::Path,
};

use alloy_primitives::Address;
use async_scoped::{Scope, TokioScope};
use brontes::{Brontes, DataBatching, PROMETHEUS_ENDPOINT_IP, PROMETHEUS_ENDPOINT_PORT};
use brontes_classifier::Classifier;
use brontes_core::decoding::Parser as DParser;
use brontes_database::clickhouse::Clickhouse;
use brontes_database_libmdbx::{
    tables::{AddressToProtocol, Tables},
    Libmdbx,
};
use brontes_inspect::{
    atomic_backrun::AtomicBackrunInspector, cex_dex::CexDexInspector, jit::JitInspector,
    sandwich::SandwichInspector, Inspector,
};
use brontes_metrics::{prometheus_exporter::initialize, PoirotMetricsListener};
use clap::Parser;
use itertools::Itertools;
use metrics_process::Collector;
use reth_db::transaction::DbTx;
use reth_tracing_ext::TracingClient;
use tokio::{pin, runtime, sync::mpsc::unbounded_channel};
use tracing::{error, info, Level};
use tracing_subscriber::{prelude::__tracing_subscriber_SubscriberExt, EnvFilter, Layer, Registry};
mod banner;
mod cli;

use banner::print_banner;
use cli::{Args, Commands, Init, Run, RunBatchWithPricing};

type Inspectors<'a> = [&'a Box<dyn Inspector>; 4];

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
        [&self.sandwich, &self.cex_dex, &self.jit, &self.backrun]
    }
}

//TODO: Wire in price fetcher + Metadata fetcher

fn main() {
    print_banner();
    dotenv::dotenv().ok();

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    let filter = EnvFilter::builder()
        .with_default_directive(Level::INFO.into())
        .from_env_lossy();

    let subscriber = Registry::default().with(tracing_subscriber::fmt::layer().with_filter(filter));

    tracing::subscriber::set_global_default(subscriber)
        .expect("Could not set global default subscriber");

    match runtime.block_on(run()) {
        Ok(()) => info!("SUCCESS!"),
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
        Commands::Run(command) => run_brontes(command).await,
        Commands::Init(command) => init_brontes(command).await,
        Commands::RunBatchWithPricing(command) => run_batch_with_pricing(command).await,
    }
}

async fn run_brontes(run_config: Run) -> Result<(), Box<dyn Error>> {
    // Fetch required environment variables.
    let db_path = get_env_vars()?;

    let max_tasks = determine_max_tasks(run_config.max_tasks);

    let (metrics_tx, metrics_rx) = unbounded_channel();

    let metrics_listener = PoirotMetricsListener::new(metrics_rx);

    let brontes_db_endpoint = env::var("BRONTES_DB_PATH").expect("No BRONTES_DB_PATH in .env");
    let libmdbx =
        Box::leak(Box::new(Libmdbx::init_db(brontes_db_endpoint, None)?)) as &'static Libmdbx;
    let clickhouse = Clickhouse::default();

    let inspector_holder = Box::leak(Box::new(InspectorHolder::new(
        run_config.quote_asset.parse().unwrap(),
        &libmdbx,
    )));

    let inspectors: Inspectors = inspector_holder.get_inspectors();

    let (mut manager, tracer) =
        TracingClient::new(Path::new(&db_path), tokio::runtime::Handle::current(), max_tasks);

    let parser = DParser::new(
        metrics_tx,
        &libmdbx,
        tracer,
        Box::new(|address, db_tx| db_tx.get::<AddressToProtocol>(*address).unwrap().is_none()),
    );

    let (tx, _rx) = tokio::sync::mpsc::unbounded_channel();
    let classifier = Classifier::new(&libmdbx, tx.clone());

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

    pin!(brontes);
    pin!(metrics_listener);

    // wait for completion
    tokio::select! {
        _ = &mut brontes => {
            info!("finnished running brontes, shutting down");
        }
        _ = Pin::new(&mut manager) => {
        }
        _ = &mut metrics_listener => {
        }
    }
    manager.graceful_shutdown();

    Ok(())
}

async fn init_brontes(init_config: Init) -> Result<(), Box<dyn Error>> {
    let brontes_db_endpoint = env::var("BRONTES_DB_PATH").expect("No BRONTES_DB_PATH in .env");

    let clickhouse = Clickhouse::default();

    let libmdbx = Libmdbx::init_db(brontes_db_endpoint, None)?;
    if init_config.init_libmdbx {
        // currently inits all tables
        let range =
            if let (Some(start), Some(end)) = (init_config.start_block, init_config.end_block) {
                Some((start, end))
            } else {
                None
            };
        libmdbx
            .clear_and_initialize_tables(
                &clickhouse,
                init_config
                    .tables_to_init
                    .unwrap_or({
                        if init_config.download_dex_pricing {
                            Tables::ALL.to_vec()
                        } else {
                            Tables::ALL_NO_DEX.to_vec()
                        }
                    })
                    .as_slice(),
                range,
            )
            .await?;
    }

    //TODO: Joe, have it download the full range of metadata from the MEV DB so
    // they can run everything in parallel
    Ok(())
}

async fn run_batch_with_pricing(config: RunBatchWithPricing) -> Result<(), Box<dyn Error>> {
    assert!(config.start_block <= config.end_block);

    let db_path = get_env_vars()?;

    let max_tasks = determine_max_tasks(config.max_tasks);

    let (metrics_tx, metrics_rx) = unbounded_channel();

    let metrics_listener = PoirotMetricsListener::new(metrics_rx);

    let brontes_db_endpoint = env::var("BRONTES_DB_PATH").expect("No BRONTES_DB_PATH in .env");
    let libmdbx =
        Box::leak(Box::new(Libmdbx::init_db(brontes_db_endpoint, None)?)) as &'static Libmdbx;

    let inspector_holder =
        Box::leak(Box::new(InspectorHolder::new(config.quote_asset.parse().unwrap(), &libmdbx)));
    let inspectors: Inspectors = inspector_holder.get_inspectors();

    let (mut manager, tracer) =
        TracingClient::new(Path::new(&db_path), tokio::runtime::Handle::current(), max_tasks);

    let parser = DParser::new(
        metrics_tx,
        &libmdbx,
        tracer,
        Box::new(|address, db_tx| db_tx.get::<AddressToProtocol>(*address).unwrap().is_none()),
    );

    let cpus = determine_max_tasks(config.max_tasks);

    let range = config.end_block - config.start_block;
    let cpus_min = range / config.min_batch_size + 1;

    let mut scope: TokioScope<'_, ()> = unsafe { Scope::create() };

    let cpus = std::cmp::min(cpus_min, cpus);

    let chunk_size = range / cpus;

    for (i, mut chunk) in (config.start_block..=config.end_block)
        .chunks(chunk_size.try_into().unwrap())
        .into_iter()
        .enumerate()
    {
        let start_block = chunk.next().unwrap();
        let end_block = chunk.last().unwrap();

        scope.spawn(spawn_batches(
            config.quote_asset.parse().unwrap(),
            0,
            i as u64,
            start_block,
            end_block,
            &parser,
            libmdbx,
            &inspectors,
        ));
    }

    // let range

    pin!(metrics_listener);
    let mut fut = Box::pin(scope.collect());

    // wait for completion
    tokio::select! {
        _ = &mut fut => {
            info!("finnished running all batch , shutting down");
        }
        _ = Pin::new(&mut manager) => {
        }
        _ = &mut metrics_listener => {
        }
    }
    manager.graceful_shutdown();

    Ok(())
}

async fn spawn_batches(
    quote_asset: Address,
    run_id: u64,
    batch_id: u64,
    start_block: u64,
    end_block: u64,
    parser: &DParser<'_, TracingClient>,
    libmdbx: &Libmdbx,
    inspectors: &Inspectors<'_>,
) {
    DataBatching::new(
        quote_asset,
        run_id,
        batch_id,
        start_block,
        end_block,
        &parser,
        &libmdbx,
        &inspectors,
    )
    .await
}

fn determine_max_tasks(max_tasks: Option<u64>) -> u64 {
    match max_tasks {
        Some(max_tasks) => max_tasks as u64,
        None => {
            let cpus = num_cpus::get_physical();
            (cpus as f64 * 0.8) as u64 // 80% of physical cores
        }
    }
}

async fn initalize_prometheus() {
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
