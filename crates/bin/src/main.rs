use colored::Colorize;
use poirot_core::{decoding::parser::Parser, init_block, success_all, success_block};
use reth_primitives::{BlockId, BlockNumberOrTag::Number};
use reth_rpc_types::trace::parity::{TraceResultsWithTransactionHash, TraceType};
use reth_tracing::TracingClient;
use tracing::{info, Level};
use tracing_subscriber::{prelude::__tracing_subscriber_SubscriberExt, EnvFilter, Layer, Registry};

//Std
use std::{collections::HashSet, env, error::Error, path::Path};

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

    let tracer = TracingClient::new(Path::new(&db_path), handle);

    let mut parser = Parser::new(key.clone());

    let (start_block, end_block) = ();  //(17795047,	17795048); //(17788433, 17788434);
    for i in start_block..end_block {
        init_block!(i, start_block, end_block);
        let block_trace: Vec<TraceResultsWithTransactionHash> =
            trace_block(&tracer, i).await.unwrap();
        let _action = parser.parse_block(i, block_trace).await;
        success_block!(i);
    }
    success_all!(start_block, end_block, 3);

    Ok(())
}

async fn trace_block(
    tracer: &TracingClient,
    block_number: u64,
) -> Result<Vec<TraceResultsWithTransactionHash>, Box<dyn Error>> {
    let mut trace_type = HashSet::new();
    trace_type.insert(TraceType::Trace);

    let parity_trace = tracer
        .trace
        .replay_block_transactions(BlockId::Number(Number(block_number)), trace_type)
        .await
        .map_err(|e| Box::new(e) as Box<dyn Error>)?
        .unwrap();

    Ok(parity_trace)
}
