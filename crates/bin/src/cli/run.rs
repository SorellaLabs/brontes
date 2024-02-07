use std::{env, path::Path};

use brontes_core::decoding::Parser as DParser;
use brontes_database::{
    clickhouse::Clickhouse,
    libmdbx::{LibmdbxReadWriter, LibmdbxReader},
};
use brontes_inspect::Inspectors;
use brontes_metrics::PoirotMetricsListener;
use brontes_types::constants::USDT_ADDRESS_STRING;
use clap::Parser;
use tokio::sync::mpsc::unbounded_channel;

use super::{determine_max_tasks, get_env_vars, static_object};
use crate::{
    cli::{get_tracing_provider, init_inspectors},
    runner::CliContext,
    BrontesRunConfig,
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
    #[arg(long, short)]
    pub max_tasks:       Option<u64>,
    /// Optional minimum batch size
    #[arg(long, default_value = "500")]
    pub min_batch_size:  u64,
    /// Optional quote asset, if omitted it will default to USDT
    #[arg(long, short, default_value = USDT_ADDRESS_STRING)]
    pub quote_asset:     String,
    /// inspectors wanted for the run. If empty will run all inspectors
    #[arg(long, short, value_delimiter = ',')]
    pub inspectors:      Option<Vec<Inspectors>>,
    /// Centralized exchanges to consider for cex-dex inspector
    #[arg(long, short, default_values = &["Binance", "Coinbase", "Okex", "BybitSpot", "Kucoin"], value_delimiter = ',')]
    pub cex_exchanges:   Option<Vec<String>>,
    /// If the dex pricing calculation should be run, even if we have the stored
    /// dex prices.
    #[arg(long, short, default_value = "false")]
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

        let brontes_db_endpoint = env::var("BRONTES_DB_PATH").expect("No BRONTES_DB_PATH in .env");

        let libmdbx = static_object(LibmdbxReadWriter::init_db(brontes_db_endpoint, None)?);

        // verify block range validity
        if let Some(end_block) = self.end_block {
            tracing::info!("verifying libmdbx state for block range");
            if !libmdbx.valid_range_state(self.start_block, end_block)? {
                return Err(eyre::eyre!(
                    "Don't have all the libmdbx state to run the given block range. please init \
                     this range first before trying to run"
                ))
            }

            if !self.run_dex_pricing
                && !libmdbx.has_dex_pricing_for_range(self.start_block, end_block)?
            {
                return Err(eyre::eyre!(
                    "Don't have dex pricing for the given range. please run the missing blocks \
                     with the `--run-dex-pricing` flag"
                ))
            }

            tracing::info!("verified libmdbx state");
        }

        // check to make sure that we have the dex-prices for the range
        if !self.run_dex_pricing && self.end_block.is_none() {
            return Err(eyre::eyre!("need end block if we aren't running the dex pricing"))
        }

        let clickhouse = (self.end_block.is_none()).then(|| static_object(Clickhouse::default()));
        let inspectors = init_inspectors(quote_asset, libmdbx, self.inspectors, self.cex_exchanges);

        let tracer = get_tracing_provider(Path::new(&db_path), max_tasks, task_executor.clone());

        let parser = static_object(DParser::new(
            metrics_tx,
            libmdbx,
            tracer.clone(),
            Box::new(|address, db_tx| db_tx.get_protocol(*address).is_err()),
        ));

        BrontesRunConfig::new(
            self.start_block,
            self.end_block,
            max_tasks,
            self.min_batch_size,
            quote_asset,
            self.run_dex_pricing,
            inspectors,
            clickhouse,
            parser,
            libmdbx,
        )
        .build(task_executor)
        .await
        .await;

        Ok(())
    }
}
