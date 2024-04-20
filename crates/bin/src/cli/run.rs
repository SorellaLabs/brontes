use std::path::Path;

use brontes_core::decoding::Parser as DParser;
use brontes_database::clickhouse::cex_config::CexDownloadConfig;
use brontes_inspect::Inspectors;
use brontes_metrics::PoirotMetricsListener;
use brontes_types::{constants::USDT_ADDRESS_STRING, db::cex::CexExchange, init_threadpools};
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
    pub start_block:            Option<u64>,
    /// Optional End Block, if omitted it will run historically & at tip until
    /// killed
    #[arg(long, short)]
    pub end_block:              Option<u64>,
    /// Optional Max Tasks, if omitted it will default to 80% of the number of
    /// physical cores on your machine
    #[arg(long, short)]
    pub max_tasks:              Option<u64>,
    /// Optional minimum batch size
    #[arg(long, default_value = "500")]
    pub min_batch_size:         u64,
    /// Optional quote asset, if omitted it will default to USDT
    #[arg(long, short, default_value = USDT_ADDRESS_STRING)]
    pub quote_asset:            String,
    /// Inspectors to run. If omitted it defaults to running all inspectors
    #[arg(long, short, value_delimiter = ',')]
    pub inspectors:             Option<Vec<Inspectors>>,
    #[cfg(not(feature = "cex-dex-markout"))]
    /// The sliding time window (BEFORE) for cex prices relative to the block
    /// timestamp
    #[arg(long = "price-tw-before", default_value = "0.5")]
    pub cex_time_window_before: f64,
    #[cfg(not(feature = "cex-dex-markout"))]
    /// The sliding time window (AFTER) for cex prices relative to the block
    /// timestamp
    #[arg(long = "price-tw-after", default_value = "2.0")]
    pub cex_time_window_after:  f64,
    #[cfg(feature = "cex-dex-markout")]
    /// The sliding time window (BEFORE) for cex trades relative to the block
    /// timestamp
    #[arg(long = "trades-tw-before", default_value = "1.0")]
    pub cex_time_window_before: f64,
    #[cfg(feature = "cex-dex-markout")]
    /// The sliding time window (AFTER) for cex trades relative to the block
    /// timestamp
    #[arg(long = "trades-tw-after", default_value = "3.0")]
    pub cex_time_window_after:  f64,
    /// Centralized exchanges to consider for cex-dex inspector
    #[arg(
        long,
        short,
        default_value = "Binance,Coinbase,Okex,BybitSpot,Kucoin",
        value_delimiter = ','
    )]
    pub cex_exchanges:          Vec<CexExchange>,
    /// Ensures that dex prices are calculated at every block, even if the
    /// db already contains the price
    #[arg(long, short, default_value = "false")]
    pub force_dex_pricing:      bool,
    /// Turns off dex pricing entirely, inspectors requiring dex pricing won't
    /// calculate USD pnl if we don't have dex pricing in the db & will only
    /// calculate token pnl
    #[arg(long, default_value = "false")]
    pub force_no_dex_pricing:   bool,
    /// How many blocks behind chain tip to run.
    #[arg(long, default_value = "3")]
    pub behind_tip:             u64,
    #[arg(long, default_value = "false")]
    pub cli_only:               bool,
    #[arg(long, default_value = "false")]
    pub init_crit_tables:       bool,
}

impl RunArgs {
    pub async fn execute(
        mut self,
        brontes_db_endpoint: String,
        ctx: CliContext,
    ) -> eyre::Result<()> {
        banner::print_banner();
        // Fetch required environment variables.
        let reth_db_path = get_env_vars()?;
        tracing::info!(target: "brontes", "got env vars");
        let quote_asset = self.quote_asset.parse()?;
        tracing::info!(target: "brontes", "parsed quote asset");
        let task_executor = ctx.task_executor;

        let max_tasks = determine_max_tasks(self.max_tasks);
        init_threadpools(max_tasks as usize);

        let (metrics_tx, metrics_rx) = unbounded_channel();
        let metrics_listener = PoirotMetricsListener::new(metrics_rx);
        task_executor.spawn_critical("metrics", metrics_listener);

        tracing::info!(target: "brontes", "starting database initialization at: '{}'", brontes_db_endpoint);
        let libmdbx = static_object(load_database(brontes_db_endpoint)?);
        tracing::info!(target: "brontes", "initialized libmdbx database");

        let cex_download_config = CexDownloadConfig::new(
            (self.cex_time_window_before, self.cex_time_window_after),
            self.cex_exchanges.clone(),
        );
        let clickhouse = static_object(load_clickhouse(cex_download_config).await?);
        tracing::info!(target: "brontes", "Databases initialized");

        let only_cex_dex = self
            .inspectors
            .as_ref()
            .map(|f| {
                #[cfg(not(feature = "cex-dex-markout"))]
                let cmp = Inspectors::CexDex;
                #[cfg(feature = "cex-dex-markout")]
                let cmp = Inspectors::CexDexMarkout;
                f.len() == 1 && f.contains(&cmp)
            })
            .unwrap_or(false);

        if only_cex_dex {
            self.force_no_dex_pricing = true;
        }

        let inspectors = init_inspectors(quote_asset, libmdbx, self.inspectors, self.cex_exchanges);
        let tracer =
            get_tracing_provider(Path::new(&reth_db_path), max_tasks, task_executor.clone());

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
                    self.force_dex_pricing,
                    self.force_no_dex_pricing,
                    inspectors,
                    clickhouse,
                    parser,
                    libmdbx,
                    self.cli_only,
                    self.init_crit_tables,
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
