use std::{env, error::Error, path::Path};
use tokio::runtime;
use env_logger::{Builder as EnvLoggerBuilder, Env, fmt::TimestampPrecision};
use log::LevelFilter;
use poirot_core::{decode::Parser, trace::TracingClient, action::Action, normalize::Normalizer};
use reth_primitives::{BlockId, BlockNumberOrTag::Number};


fn main() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_stack_size(8 * 1024 * 1024)
        .build()
        .unwrap();

    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_module_path(false)
        .format_timestamp(Some(env_logger::fmt::TimestampPrecision::Millis))
        .filter_module(module_path!(), log::LevelFilter::Debug)
        .init();

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

    let key = match env::var("ETHERSCAN_API_KEY") {
        Ok(key) => key,
        Err(_) => return Err(Box::new(std::env::VarError::NotPresent)),
    };

    let tracer = TracingClient::new(Path::new(&db_path), handle);

    let parity_trace =
        tracer.trace.trace_block(BlockId::Number(Number(17679852))).await.unwrap().unwrap();

    let mut parser = Parser::new(parity_trace, key);

    let actions = parser.parse().await;
    parser.stats.display();

    let normalizer = Normalizer::new(actions).normalize();

    for structure in normalizer {
        for val in structure {
            println!("{:#?}", val);
        }
    }

    Ok(())
}
