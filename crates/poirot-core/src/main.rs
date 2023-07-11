use ethers::prelude::k256::elliptic_curve::rand_core::block;
use poirot_core::{TracingClient};
use std::{env, error::Error, path::Path};
use reth_rpc_types::trace::parity::TraceType;
use std::collections::HashSet;
use reth_primitives::{BlockId, BlockNumberOrTag};
use tracing::Subscriber;
use tracing_subscriber::{
    filter::Directive, prelude::*, registry::LookupSpan, EnvFilter, Layer, Registry,
};
use reth_primitives::{BlockNumHash, H256};
use reth_rpc_types::trace::geth::GethDebugTracingOptions;
use alloy_json_abi::*;
use std::str::FromStr;

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

    let db_path = Path::new(&db_path);

    let tracer = TracingClient::new(db_path, handle);

    Ok(())
}