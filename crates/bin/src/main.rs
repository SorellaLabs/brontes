use std::{
    collections::HashMap,
    env,
    error::Error,
    net::{IpAddr, Ipv4Addr, SocketAddr}
};

use bin::{Poirot, PROMETHEUS_ENDPOINT_IP, PROMETHEUS_ENDPOINT_PORT};
use colored::Colorize;
use metrics_process::Collector;
use poirot_classifer::Classifier;
use poirot_core::{decoding::Parser, init_block};
use poirot_inspect::atomic_backrun::AtomicBackrunInspector;
use poirot_labeller::{database::Database, Labeller};
use poirot_metrics::{prometheus_exporter::initialize, PoirotMetricsListener};
use tokio::sync::mpsc::unbounded_channel;
use tracing::{info, Level};
use tracing_subscriber::{prelude::__tracing_subscriber_SubscriberExt, EnvFilter, Layer, Registry};

fn main() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_stack_size(8 * 1024 * 1024)
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
    // initializes the prometheus endpoint
    initialize(
        SocketAddr::new(
            IpAddr::V4(Ipv4Addr::from(PROMETHEUS_ENDPOINT_IP)),
            PROMETHEUS_ENDPOINT_PORT
        ),
        Collector::default()
    )
    .await
    .unwrap();
    info!("Initialized prometheus endpoint");

    let db_path = match env::var("DB_PATH") {
        Ok(path) => path,
        Err(_) => return Err(Box::new(std::env::VarError::NotPresent))
    };

    info!("Found DB Path");

    let key = match env::var("ETHERSCAN_API_KEY") {
        Ok(key) => key,
        Err(_) => return Err(Box::new(std::env::VarError::NotPresent))
    };
    info!("Found Etherscan API Key");

    let (metrics_tx, metrics_rx) = unbounded_channel();
    let metrics_listener =
        tokio::spawn(async move { PoirotMetricsListener::new(metrics_rx).await });

    let dummy_inspector = Box::new(AtomicBackrunInspector);
    let inspectors = &[&dummy_inspector];
    let db = Database::default();
    let poirot_labeller = Labeller::new(metrics_tx.clone(), &db);
    let parser = Parser::new(metrics_tx, &key, &db_path);
    let classifier = Classifier::new(HashMap::default());

    Poirot::new(parser, poirot_labeller, classifier, inspectors, 69420).await;

    // you have a intermediate parse function for the range of blocks you want to
    // parse it collects the aggregate stats of each block stats
    // the block stats collect the aggregate stats of each tx
    // the tx stats collect the aggregate stats of each trace

    metrics_listener.await?;
    Ok(())
}
