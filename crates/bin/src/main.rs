use bin::{prometheus_exporter::initialize, *};
use colored::Colorize;
use metrics_process::Collector;
use poirot_core::{decoding::parser::Parser, init_block, success_block, stats::TraceMetricsListener};
use reth_rpc_types::trace::parity::TraceResultsWithTransactionHash;
use reth_tracing::TracingClient;
use tokio::sync::mpsc::unbounded_channel;
use tracing::{info, Level};
use tracing_subscriber::{prelude::__tracing_subscriber_SubscriberExt, EnvFilter, Layer, Registry};

//Std
use std::{
    env,
    error::Error,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::Path,
};

fn main() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_stack_size(8 * 1024 * 1024)
        .build()
        .unwrap();

    let filter = EnvFilter::builder().with_default_directive(Level::INFO.into()).from_env_lossy();

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

async fn run(handle: tokio::runtime::Handle) -> Result<(), Box<dyn Error>> {
    // initializes the prometheus endpoint
    initialize(SocketAddr::new(
        IpAddr::V4(Ipv4Addr::from(PROMETHEUS_ENDPOINT_IP)),
        PROMETHEUS_ENDPOINT_PORT,
    ), 
    Collector::default()
    ).await.unwrap();
    info!("Initialized prometheus endpoint");


    let db_path = match env::var("DB_PATH") {
        Ok(path) => path,
        Err(_) => return Err(Box::new(std::env::VarError::NotPresent)),
    };

    info!("Found DB Path");

    let key = match env::var("ETHERSCAN_API_KEY") {
        Ok(key) => key,
        Err(_) => return Err(Box::new(std::env::VarError::NotPresent)),
    };
    info!("Found Etherscan API Key");

    let (metrics_tx, metrics_rx) = unbounded_channel();
    let metrics_listener = tokio::spawn(async move { TraceMetricsListener::new(metrics_rx).await });

    let tracer = TracingClient::new(Path::new(&db_path), handle);

    let mut parser = Parser::new(key.clone(), tracer, metrics_tx);



    // you have a intermediate parse function for the range of blocks you want to parse
    // it collects the aggregate stats of each block stats
    // the block stats collect the aggregate stats of each tx
    // the tx stats collect the aggregate stats of each trace

    let (start_block, end_block) = (17784930, 17794931); //(17795047,	17795048); //(17788433, 17788434);
    for i in start_block..end_block {
        init_block!(i, start_block, end_block);
        let block_trace: Vec<TraceResultsWithTransactionHash> =
            parser.trace_block(i).await.unwrap();
        let _action = parser.parse_block(i, block_trace).await;
        success_block!(i);
    }
    info!("Successfully Parsed Blocks {} To {} ", start_block, end_block);

    metrics_listener.await?;
    Ok(())
}
