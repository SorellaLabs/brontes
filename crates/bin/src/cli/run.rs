use std::{env, path::Path};

use brontes_core::decoding::Parser as DParser;
use brontes_inspect::Inspectors;
use brontes_metrics::PoirotMetricsListener;
use brontes_types::{constants::USDT_ADDRESS_STRING, init_threadpools};
use clap::Parser;
use tokio::sync::mpsc::unbounded_channel;

use super::{determine_max_tasks, get_env_vars, load_clickhouse, load_database, static_object};
use crate::{
    banner,
    cli::{get_tracing_provider, init_inspectors},
    runner::CliContext,
    BrontesRunConfig, MevProcessor,
};

#[derive(Debug, Parser)]
pub struct RunArgs {
    /// Optional Start Block, if omitted it will run at tip until killed
    #[arg(long, short)]
    pub start_block:     Option<u64>,
    /// Optional End Block, if omitted it will run historically & at tip until
    /// killed
    #[arg(long, short)]
    pub end_block:       Option<u64>,
    /// Optional Max Tasks, if omitted it will default to 80% of the number of
    /// physical cores on your machine
    #[arg(long, short)]
    pub max_tasks:       Option<u64>,
    /// Optional minimum batch size
    #[arg(long, default_value = "500")]
    pub min_batch_size:  u64,
    /// Optional quote asset, if omitted it will default to USDT
    #[arg(long, short, default_value = USDT_ADDRESS_STRING)]
    pub quote_asset:     String,
    /// Inspectors to run. If omitted it defaults to running all inspectors
    #[arg(long, short, value_delimiter = ',')]
    pub inspectors:      Option<Vec<Inspectors>>,
    /// Centralized exchanges to consider for cex-dex inspector
    #[arg(long, short, default_values = &["Binance", "Coinbase", "Okex", "BybitSpot", "Kucoin"], value_delimiter = ',')]
    pub cex_exchanges:   Vec<String>,
    /// If the dex pricing calculation should be run, even if we have the stored
    /// dex prices.
    #[arg(long, short, default_value = "false")]
    pub run_dex_pricing: bool,
    /// How many blocks behind chain tip to run.
    #[arg(long, default_value = "3")]
    pub behind_tip:      u64,
}

impl RunArgs {
    pub async fn execute(self, ctx: CliContext) -> eyre::Result<()> {
        banner::print_banner();
        // Fetch required environment variables.
        let db_path = get_env_vars()?;
        tracing::info!(target: "brontes", "got env vars");
        let quote_asset = self.quote_asset.parse()?;
        tracing::info!(target: "brontes", "parsed quote asset");
        let task_executor = ctx.task_executor;

        let max_tasks = determine_max_tasks(self.max_tasks);
        init_threadpools(max_tasks as usize);

        let (metrics_tx, metrics_rx) = unbounded_channel();
        let metrics_listener = PoirotMetricsListener::new(metrics_rx);
        task_executor.spawn_critical("metrics", metrics_listener);

        let brontes_db_endpoint = env::var("BRONTES_DB_PATH").expect("No BRONTES_DB_PATH in .env");

        tracing::info!(target: "brontes", "starting database initialization");
        let libmdbx = static_object(load_database(brontes_db_endpoint)?);
        tracing::info!(target: "brontes", "Initialize Libmdbx");
        let clickhouse = static_object(load_clickhouse().await?);
        tracing::info!(target: "brontes", "Databases initialized");

        let inspectors = init_inspectors(quote_asset, libmdbx, self.inspectors, self.cex_exchanges);

        let tracer = get_tracing_provider(Path::new(&db_path), max_tasks, task_executor.clone());

        let parser = static_object(DParser::new(metrics_tx, libmdbx, tracer.clone()).await);

        let executor = task_executor.clone();
        let result = executor
            .clone()
            .spawn_critical_with_graceful_shutdown_signal("run init", |shutdown| async move {
                if let Ok(brontes) = BrontesRunConfig::<_, _, _, MevProcessor>::new(
                    self.start_block,
                    self.end_block,
                    self.behind_tip,
                    max_tasks,
                    self.min_batch_size,
                    quote_asset,
                    self.run_dex_pricing,
                    inspectors,
                    clickhouse,
                    parser,
                    libmdbx,
                )
                .build(task_executor, shutdown)
                .await
                .map_err(|e| {
                    tracing::error!(%e);
                    e
                }) {
                    brontes.await;
                }
            });

        result.await?;

        Ok(())
    }
}
