use std::{
    env,
    error::Error,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use alloy_primitives::Address;
use async_scoped::{Scope, TokioScope};
use brontes_classifier::Classifier;
use brontes_core::decoding::Parser as DParser;
use brontes_database::{
    clickhouse::Clickhouse,
    libmdbx::{
        cursor::CompressedCursor,
        tables::{AddressToProtocol, CompressedTable, IntoTableKey, Tables},
        Libmdbx,
    },
};
use brontes_inspect::{
    atomic_backrun::AtomicBackrunInspector, cex_dex::CexDexInspector, jit::JitInspector,
    sandwich::SandwichInspector, Inspector,
};
use brontes_metrics::{prometheus_exporter::initialize, PoirotMetricsListener};
use clap::Parser;
use itertools::Itertools;
use metrics_process::Collector;
use reth_db::mdbx::RO;
use reth_tracing_ext::TracingClient;
use tokio::sync::mpsc::unbounded_channel;
use tracing::{error, info, Level};
use tracing_subscriber::filter::Directive;

use super::{determine_max_tasks, get_env_vars, init_all_inspectors};
use crate::{Brontes, DataBatching, PROMETHEUS_ENDPOINT_IP, PROMETHEUS_ENDPOINT_PORT};

#[derive(Debug, Parser)]
pub struct DexPricingArgs {
    #[arg(long, short)]
    pub start_block:    u64,
    /// Optional End Block, if omitted it will continue to run until killed
    #[arg(long, short)]
    pub end_block:      u64,
    /// Optional Max Tasks, if omitted it will default to 50% of the number of
    /// physical cores on your machine
    pub max_tasks:      Option<u64>,
    /// Optional quote asset, if omitted it will default to USDC
    #[arg(long, short, default_value = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48")]
    pub quote_asset:    String,
    /// how big the batch size should be
    #[arg(long, short, default_value = "500")]
    pub min_batch_size: u64,
}
impl DexPricingArgs {
    pub async fn execute(self) -> Result<(), Box<dyn Error>> {
        assert!(self.start_block <= self.end_block);
        info!(?self);

        let db_path = get_env_vars()?;
        let quote = self.quote_asset.parse()?;

        let tracing_max_tasks = determine_max_tasks(self.max_tasks);
        let (metrics_tx, metrics_rx) = unbounded_channel();

        let metrics_listener = PoirotMetricsListener::new(metrics_rx);
        tokio::spawn(metrics_listener);

        let brontes_db_endpoint = env::var("BRONTES_DB_PATH").expect("No BRONTES_DB_PATH in .env");
        let libmdbx =
            Box::leak(Box::new(Libmdbx::init_db(brontes_db_endpoint, None)?)) as &'static Libmdbx;

        let inspectors = init_all_inspectors(quote, libmdbx);

        let (manager, tracer) = TracingClient::new(
            Path::new(&db_path),
            tokio::runtime::Handle::current(),
            tracing_max_tasks,
        );

        tokio::spawn(manager);

        let parser = DParser::new(
            metrics_tx,
            &libmdbx,
            tracer,
            Box::new(|address, db_tx| db_tx.get::<AddressToProtocol>(*address).unwrap().is_none()),
        );

        let mut scope: TokioScope<'_, ()> = unsafe { Scope::create() };

        // calculate the chunk size using min batch size and max_tasks.
        // max tasks defaults to 50% of physical threads of the system if not set
        let cpus = determine_max_tasks(self.max_tasks);
        let range = self.end_block - self.start_block;
        let cpus_min = range / self.min_batch_size;

        let cpus = std::cmp::min(cpus_min, cpus);
        let chunk_size = if cpus == 0 { range + 1 } else { (range / cpus) + 1 };

        let remaining_cpus = if self.max_tasks.is_some() {
            determine_max_tasks(None) * 2 - self.max_tasks.unwrap()
        } else {
            determine_max_tasks(None)
        };

        let chunks_amount = (range / chunk_size) + 1;
        // because these are lightweight tasks, we can stack them pretty easily without
        // much overhead concern
        let max_pool_loading_tasks = (remaining_cpus / chunks_amount + 1) * 3;

        for (i, mut chunk) in (self.start_block..=self.end_block)
            .chunks(chunk_size.try_into().unwrap())
            .into_iter()
            .enumerate()
        {
            let start_block = chunk.next().unwrap();
            let end_block = chunk.last().unwrap_or(start_block);

            info!(batch_id = i, start_block, end_block, "starting batch");

            scope.spawn(spawn_batches(
                self.quote_asset.parse().unwrap(),
                max_pool_loading_tasks as usize,
                i as u64,
                start_block,
                end_block,
                &parser,
                libmdbx,
                &inspectors,
            ));
        }

        // collect and wait
        scope.collect().await;

        info!("finnished running all batch , shutting down");
        drop(scope);
        std::thread::spawn(move || {
            drop(parser);
        });

        Ok(())
    }
}

async fn spawn_batches<'a>(
    quote_asset: Address,
    max_pool_loading_tasks: usize,
    batch_id: u64,
    start_block: u64,
    end_block: u64,
    parser: &DParser<'static, TracingClient>,
    libmdbx: &'static Libmdbx,
    inspectors: &[&'static Box<dyn Inspector>],
) {
    DataBatching::new(
        quote_asset,
        max_pool_loading_tasks,
        batch_id,
        start_block,
        end_block,
        &parser,
        &libmdbx,
        &inspectors,
    )
    .await
}
