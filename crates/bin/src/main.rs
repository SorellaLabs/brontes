use std::{
    collections::HashMap,
    env,
    error::Error,
    net::{IpAddr, Ipv4Addr, SocketAddr},
};

use bin::{Poirot, PROMETHEUS_ENDPOINT_IP, PROMETHEUS_ENDPOINT_PORT};
use metrics_process::Collector;
use poirot_classifier::Classifier;
use poirot_core::decoding::Parser;
use poirot_database::database::Database;
use poirot_inspect::{atomic_backrun::AtomicBackrunInspector, composer::Composer, Inspector};
use poirot_metrics::{prometheus_exporter::initialize, PoirotMetricsListener};
use tokio::sync::mpsc::unbounded_channel;
use tracing::{info, Level};
use tracing_subscriber::{prelude::__tracing_subscriber_SubscriberExt, EnvFilter, Layer, Registry};
mod cli;

use cli::{print_banner, Commands};

fn main() {
    print_banner();

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

    match runtime.block_on(run(runtime.handle().clone())) {
        Ok(()) => info!("SUCCESS!"),
        Err(e) => {
            eprintln!("Error: {:?}", e);

            let mut source: Option<&dyn Error> = e.source();
            while let Some(err) = source {
                eprintln!("Caused by: {:?}", err);
                source = err.source();
            }
        }
    }
}

async fn run(_handle: tokio::runtime::Handle) -> Result<(), Box<dyn Error>> {
    // parse cli
    let opt = Commands::parse();

    initalize_prometheus().await;

    // Fetch required environment variables.
    let (db_path, etherscan_key) = get_env_vars()?;

    let (metrics_tx, metrics_rx) = unbounded_channel();

    let metrics_listener =
        tokio::spawn(async move { PoirotMetricsListener::new(metrics_rx).await });

    let dummy_inspector = Box::new(AtomicBackrunInspector {}) as Box<dyn Inspector>;
    let baby_inspectors = &[&dummy_inspector];

    let daddy_inspector = DaddyInspector::new(baby_inspectors);

    let db = Database::default();
    let parser = Parser::new(metrics_tx, &etherscan_key, &db_path);
    let classifier = Classifier::new(HashMap::default());

    Poirot::new(parser, &db, classifier, daddy_inspector, command.start_block, command.end_block)
        .await;

    // you have a intermediate parse function for the range of blocks you want to
    // parse it collects the aggregate stats of each block stats
    // the block stats collect the aggregate stats of each tx
    // the tx stats collect the aggregate stats of each trace

    metrics_listener.await?;
    Ok(())
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

fn get_env_vars() -> Result<(String, String), Box<dyn Error>> {
    let db_path = env::var("DB_PATH").map_err(|_| Box::new(std::env::VarError::NotPresent))?;
    info!("Found DB Path");

    let etherscan_key =
        env::var("ETHERSCAN_API_KEY").map_err(|_| Box::new(std::env::VarError::NotPresent))?;
    info!("Found Etherscan API Key");

<<<<<<< HEAD
    Ok((db_path, key))
=======
    let (metrics_tx, metrics_rx) = unbounded_channel();
    let metrics_listener =
        tokio::spawn(async move { PoirotMetricsListener::new(metrics_rx).await });

    let dummy_inspector = Box::new(AtomicBackrunInspector {}) as Box<dyn Inspector>;
    let orchestra = &[&dummy_inspector];

    let composer = Composer::new(orchestra);

    let db = Database::default();
    let parser = Parser::new(metrics_tx, &key, &db_path);
    let classifier = Classifier::new(HashMap::default());

    Poirot::new(parser, &db, classifier, composer, 69420).await;

    // you have a intermediate parse function for the range of blocks you want to
    // parse it collects the aggregate stats of each block stats
    // the block stats collect the aggregate stats of each tx
    // the tx stats collect the aggregate stats of each trace

    metrics_listener.await?;
    Ok(())
>>>>>>> 62fc500249aade60d64c9dc043022ae5dcd89442
}
