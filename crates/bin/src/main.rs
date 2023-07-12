use poirot_core::trace::TracingClient;
use poirot_core::decode::Parser;

use std::{env, error::Error, path::Path};

use reth_primitives::{BlockId, BlockNumberOrTag};

fn main() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_stack_size(8 * 1024 * 1024)
        .build()
        .unwrap();

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

    let key = match env::var("ETHERSCAN_API") {
        Ok(key) => key,
        Err(_) => return Err(Box::new(std::env::VarError::NotPresent)),
    };

    let tracer = TracingClient::new(Path::new(&db_path), handle);

    let parity_trace =
        tracer.trace.trace_block(BlockId::Number(BlockNumberOrTag::Latest)).await?;

    let parser = Parser::new(parity_trace, key);

    println!("{:#?}", parser.parse().await);

    Ok(())
}
