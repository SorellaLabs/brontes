use poirot_core::{decode::Parser, stats::ParserStatsLayer};
use reth_primitives::{BlockId, BlockNumberOrTag::Number};
use reth_rpc_types::trace::parity::{TraceResultsWithTransactionHash, TraceType};
use tracing_subscriber::{Registry, prelude::__tracing_subscriber_SubscriberExt, util::SubscriberInitExt};
use reth_tracing::TracingClient;


//Std
use std::{collections::HashSet, env, error::Error, path::Path};

fn main() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_stack_size(8 * 1024 * 1024)
        .build()
        .unwrap();

    let subscriber = Registry::default()
        .with(ParserStatsLayer)
        .with(tracing_subscriber::fmt::layer());
    
    tracing::subscriber::set_global_default(subscriber)
        .expect("Could not set global default subscriber");
    

    match runtime.block_on(run(runtime.handle().clone())) {
        Ok(()) => println!("Success!"),
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

    println!("found db path");

    let key = match env::var("ETHERSCAN_API_KEY") {
        Ok(key) => key,
        Err(_) => return Err(Box::new(std::env::VarError::NotPresent)),
    };
    println!("found etherscan api key");

    let tracer = TracingClient::new(Path::new(&db_path), handle);

    let mut parser = Parser::new(key.clone());

    let block_trace = trace_block(&tracer, 17679852).await.unwrap();
    let action = parser.parse_block(17679852, block_trace).await;

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