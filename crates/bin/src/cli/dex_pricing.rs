use std::{env, path::Path};

use brontes_classifier::Classifier;
use brontes_core::decoding::Parser as DParser;
#[cfg(feature = "local")]
use brontes_core::local_provider::LocalProvider;
use brontes_database::libmdbx::{tables::AddressToProtocol, LibmdbxReadWriter, LibmdbxReader};
use brontes_metrics::PoirotMetricsListener;
use clap::Parser;
use futures::stream::{FuturesUnordered, StreamExt};
use itertools::Itertools;
use reth_tracing_ext::TracingClient;
use tokio::sync::mpsc::unbounded_channel;
use tracing::info;

use super::{determine_max_tasks, get_env_vars, init_all_inspectors};
use crate::{runner::CliContext, DataBatching};

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
    pub async fn execute(self, ctx: CliContext) -> eyre::Result<()> {
        assert!(self.start_block <= self.end_block);
        info!(?self);

        let db_path = get_env_vars()?;
        let quote_asset = self.quote_asset.parse()?;

        let task_executor = ctx.task_executor;

        // if we can we want max threads for these tasks
        let tracing_max_tasks = determine_max_tasks(self.max_tasks);
        let (metrics_tx, metrics_rx) = unbounded_channel();

        let metrics_listener = PoirotMetricsListener::new(metrics_rx);
        task_executor.spawn_critical("metrics listener", metrics_listener);

        let brontes_db_endpoint = env::var("BRONTES_DB_PATH").expect("No BRONTES_DB_PATH in .env");
        let libmdbx = Box::leak(Box::new(LibmdbxReadWriter::init_db(brontes_db_endpoint, None)?))
            as &'static LibmdbxReadWriter;

        let inspectors = init_all_inspectors(quote_asset, libmdbx);

        #[cfg(not(feature = "local"))]
        let tracer =
            TracingClient::new(Path::new(&db_path), tracing_max_tasks, task_executor.clone());
        #[cfg(feature = "local")]
        let tracer = {
            let db_endpoint = env::var("RETH_ENDPOINT").expect("No db Endpoint in .env");
            let db_port = env::var("RETH_PORT").expect("No DB port.env");
            let url = format!("{db_endpoint}:{db_port}");
            LocalProvider::new(url)
        };

        let parser = &*Box::leak(Box::new(DParser::new(
            metrics_tx,
            libmdbx,
            tracer.clone(),
            Box::new(|address, db_tx: &LibmdbxReadWriter| {
                db_tx.get_protocol(*address).unwrap().is_none()
            }),
        )));

        // calculate the chunk size using min batch size and max_tasks.
        // max tasks defaults to 25% of physical threads of the system if not set
        let cpus = determine_max_tasks(self.max_tasks);
        let range = self.end_block - self.start_block;
        let cpus_min = range / self.min_batch_size;

        let cpus = std::cmp::min(cpus_min, cpus);
        let chunk_size = if cpus == 0 { range + 1 } else { (range / cpus) + 1 };

        let mut tasks = FuturesUnordered::new();

        for (batch_id, mut chunk) in (self.start_block..=self.end_block)
            .chunks(chunk_size.try_into().unwrap())
            .into_iter()
            .enumerate()
        {
            let start_block = chunk.next().unwrap();
            let end_block = chunk.last().unwrap_or(start_block);

            info!(batch_id, start_block, end_block, "starting batch");

            let ex = task_executor.clone();
            let tracer = tracer.clone();
            tasks.push(task_executor.spawn_critical_with_graceful_shutdown_signal(
                "pricing batch",
                |grace| async move {
                    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
                    let classifier = Classifier::new(libmdbx, tx.clone(), tracer.into());
                    DataBatching::new(
                        quote_asset,
                        batch_id as u64,
                        start_block,
                        end_block,
                        &parser,
                        &libmdbx,
                        &inspectors,
                        ex,
                        &classifier,
                        rx,
                    )
                    .run_until_graceful_shutdown(grace)
                    .await;
                },
            ));
        }

        while let Some(_) = tasks.next().await {}

        info!("finnished running all batch , shutting down");
        Ok(())
    }
}
