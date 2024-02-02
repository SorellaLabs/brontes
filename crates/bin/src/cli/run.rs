use std::{env, path::Path};

use brontes_classifier::Classifier;
use brontes_core::decoding::Parser as DParser;
use brontes_database::{
    clickhouse::Clickhouse,
    libmdbx::{LibmdbxReadWriter, LibmdbxReader},
};
use brontes_inspect::Inspectors;
use brontes_metrics::PoirotMetricsListener;
use clap::Parser;
use eyre::anyhow;
use tokio::sync::mpsc::unbounded_channel;
use tracing::info;

use super::{determine_max_tasks, get_env_vars, static_object};
use crate::{
    cli::{get_tracing_provider, init_inspectors},
    runner::CliContext,
    Brontes,
};

#[derive(Debug, Parser)]
pub struct RunArgs {
    /// Start Block
    #[arg(long, short)]
    pub start_block:     u64,
    /// Optional End Block, if omitted it will continue to run until killed
    #[arg(long, short)]
    pub end_block:       Option<u64>,
    /// Optional Max Tasks, if omitted it will default to 80% of the number of
    /// physical cores on your machine
    pub max_tasks:       Option<u64>,
    /// Optional quote asset, if omitted it will default to USDC
    #[arg(long, short, default_value = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48")]
    pub quote_asset:     String,
    /// inspectors wanted for the run. If empty will run all inspectors
    #[arg(long, short, value_delimiter = ',')]
    pub inspectors:      Option<Vec<Inspectors>>,
    /// Centralized exchanges to consider for cex-dex inspector
    #[arg(long, short, default_values = &["Binance", "Coinbase", "Kraken", "Bybit", "Kucoin"], value_delimiter = ',')]
    pub cex_exchanges:   Option<Vec<String>>,
    /// If we should run dex pricing, even if we have the stored dex prices.
    #[arg(long, short, default_values = false)]
    pub run_dex_pricing: bool,
}
impl RunArgs {
    pub async fn execute(self, ctx: CliContext) -> eyre::Result<()> {
        // Fetch required environment variables.
        let db_path = get_env_vars()?;
        let quote_asset = self.quote_asset.parse()?;
        let task_executor = ctx.task_executor;

        let max_tasks = determine_max_tasks(self.max_tasks);

        let (metrics_tx, metrics_rx) = unbounded_channel();

        let metrics_listener = PoirotMetricsListener::new(metrics_rx);
        task_executor.spawn_critical("metrics", metrics_listener);

        let brontes_db_endpoint = env::var("BRONTES_DB_PATH").expect(
            "No
        BRONTES_DB_PATH in .env",
        );

        let libmdbx = static_object(LibmdbxReadWriter::init_db(brontes_db_endpoint, None)?);

        // verify block range validity
        if let Some(end_block) = self.end_block {
            if !libmdbx.valid_range_state(self.start_block, end_block)? {
                return Err(eyre::eyre!(
                    "do not have all the libmdbx state to run the given block range. please init \
                     this range first before trying to run"
                ))
            }
        }

        // check to make sure that we have the dex-prices for the range
        if !self.run_dex_pricing {
            if self.end_block.is_none() {
                return Err(eyre::eyre!("need end block if we aren't running the dex pricing"))
            }
        }

        let clickhouse = static_object(Clickhouse::default());
        let inspectors =
            init_inspectors(quote_asset, libmdbx, self.inspectors_to_run, self.cex_exchanges);

        let tracer = get_tracing_provider(&Path::new(&db_path), max_tasks, task_executor.clone());

        let parser = static_object(DParser::new(
            metrics_tx,
            libmdbx,
            tracer.clone(),
            Box::new(|address, db_tx| db_tx.get_protocol(*address).unwrap().is_none()),
        ));

        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let classifier = static_object(Classifier::new(libmdbx, tx.clone(), tracer.into()));

        #[cfg(not(feature = "local"))]
        let chain_tip = parser.get_latest_block_number().unwrap();
        #[cfg(feature = "local")]
        let chain_tip = parser.get_latest_block_number().await.unwrap();

        let crit = task_executor
            .clone()
            .spawn_critical_with_graceful_shutdown_signal("Brontes", |grace| async move {
                Brontes::new(
                    self.start_block,
                    self.end_block,
                    chain_tip,
                    max_tasks.into(),
                    parser,
                    clickhouse,
                    libmdbx,
                    classifier,
                    inspectors,
                    task_executor,
                    rx,
                    quote_asset,
                )
                .run_until_graceful_shutdown(grace)
                .await
            });

        let _ = crit.await;

        info!("finnished running brontes, shutting down");
        Ok(())
    }
}
