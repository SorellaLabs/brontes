use poirot_core::{decode::Parser, trace::TracingClient};

use std::{env, error::Error, path::Path};

use reth_primitives::{BlockId, BlockNumberOrTag::Number};

fn main() {
    env_logger::init();

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

    let key = match env::var("ETHERSCAN_API_KEY") {
        Ok(key) => key,
        Err(_) => return Err(Box::new(std::env::VarError::NotPresent)),
    };

    let tracer = TracingClient::new(Path::new(&db_path), handle);

    let parity_trace =
        tracer.trace.trace_block(BlockId::Number(Number(17679852))).await.unwrap().unwrap();

    let mut parser = Parser::new(parity_trace, key);

    let actions = parser.parse().await;

    let mut tx_map: HashMap<B256, Vec<Action>> = std::collections::HashMap::new();

    for i in actions {
        match tx_map.get_mut(&i.trace.transaction_hash.unwrap()) {
            Some(vec) => vec.push(i), 
            None => tx_map.insert(i.trace.transaction_hash.unwrap(), i),
        }
    }

    parser.stats.display();


    Ok(())
}
