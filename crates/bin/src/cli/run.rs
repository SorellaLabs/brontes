use std::{path::Path, time::Duration};

use brontes_core::decoding::Parser as DParser;
use brontes_database::clickhouse::cex_config::CexDownloadConfig;
use brontes_inspect::Inspectors;
use brontes_metrics::ParserMetricsListener;
use brontes_types::{
    constants::USDT_ADDRESS_STRING,
    db::cex::{trades::CexDexTradeConfig, CexExchange},
    db_write_trigger::{backup_server_heartbeat, start_hr_monitor, HeartRateMonitor},
    init_thread_pools, UnboundedYapperReceiver,
};
use clap::Parser;
use tokio::sync::mpsc::unbounded_channel;

use super::{determine_max_tasks, get_env_vars, load_clickhouse, load_database, static_object};
use crate::{
    banner::rain,
    cli::{get_tracing_provider, init_inspectors, load_tip_database},
    runner::CliContext,
    BrontesRunConfig, MevProcessor, RangeType,
};

const SECONDS_TO_US_FLOAT: f64 = 1_000_000.0;

#[derive(Debug, Parser)]
pub struct RunArgs {
    /// Optional Start Block, if omitted it will run at tip until killed
    #[arg(long, short)]
    pub start_block:          Option<u64>,
    /// Optional End Block, if omitted it will run historically & at tip until
    /// killed
    #[arg(long, short)]
    pub end_block:            Option<u64>,
    /// starts running at tip from where brontes was last left at.
    #[arg(long, default_value_t = false)]
    pub from_db_tip:          bool,
    /// Optional Multiple Ranges, format: "start1-end1 start2-end2 ..."
    /// Use this if you want to specify the exact, non continuous block ranges
    /// you want to run
    #[arg(long, num_args = 1.., value_delimiter = ' ')]
    pub ranges:               Option<Vec<String>>,
    /// Optional Max Tasks, if omitted it will default to 80% of the number of
    /// physical cores on your machine
    #[arg(long, short = 't')]
    pub max_tasks:            Option<u64>,
    /// Optional max pending trees when processing dex price quotes and collecting states
    /// Limits the amount we work ahead in the processing. This is done
    /// as the Pricer is a slow process and otherwise we will end up caching 100+ gb
    /// of processed trees
    #[arg(long, short = 'p', default_value = "100")]
    pub max_pending: usize,

    /// Optional minimum batch size
    #[arg(long, default_value = "500")]
    pub min_batch_size:       u64,
    /// Optional quote asset, if omitted it will default to USDT
    #[arg(long, short, default_value = USDT_ADDRESS_STRING)]
    pub quote_asset:          String,
    /// Inspectors to run. If omitted it defaults to running all inspectors
    #[arg(long, short, value_delimiter = ',')]
    pub inspectors:           Option<Vec<Inspectors>>,
    /// Time window arguments for cex data downloads
    #[clap(flatten)]
    pub time_window_args:     TimeWindowArgs,
    /// CEX exchanges to consider for cex-dex analysis
    #[arg(
        long,
        short,
        default_value = "Binance,Coinbase,Okex,BybitSpot,Kucoin",
        value_delimiter = ','
    )]
    pub cex_exchanges:        Vec<CexExchange>,
    /// Force DEX price calculation for every block, ignoring existing database
    /// values.
    #[arg(long, short, default_value = "false")]
    pub force_dex_pricing:    bool,
    /// Disables DEX pricing. Inspectors needing DEX prices will only calculate
    /// token PnL, not USD PnL, if DEX pricing is unavailable in the
    /// database.
    #[arg(long, default_value = "false")]
    pub force_no_dex_pricing: bool,
    /// Number of blocks to lag behind the chain tip when processing.
    #[arg(long, default_value = "10")]
    pub behind_tip:           u64,
    /// Legacy, run in CLI only mode (no TUI) - will output progress bars to
    /// stdout
    #[arg(long, default_value = "true")]
    pub cli_only:             bool,
    /// Export metrics
    #[arg(long, default_value = "false")]
    pub with_metrics:         bool,
    /// Wether or not to use a fallback server.
    #[arg(long, default_value_t = false)]
    pub enable_fallback:      bool,
    /// Address of the fallback server.
    /// Triggers database writes if the main connection fails, preventing data
    /// loss.
    #[arg(long)]
    pub fallback_server:      Option<String>,
    /// Set a custom run ID used when inserting data into the Clickhouse
    ///
    /// If omitted, the ID will be automatically incremented from the last run
    /// stored in the Clickhouse database.
    #[arg(long, short)]
    pub run_id:               Option<u64>,

    /// shows a cool display at startup
    #[arg(long, short, default_value_t = false)]
    pub waterfall: bool,
}

impl RunArgs {
    pub async fn execute(mut self, brontes_db_path: String, ctx: CliContext) -> eyre::Result<()> {
        self.check_proper_range()?;

        if self.waterfall {
            rain();
        }

        let snapshot_mode = !cfg!(feature = "local-clickhouse");
        tracing::info!(%snapshot_mode);

        // Fetch required environment variables.
        let reth_db_path = get_env_vars()?;
        tracing::info!(target: "brontes", "got env vars");
        let quote_asset = self.quote_asset.parse()?;
        tracing::info!(target: "brontes", "parsed quote asset");
        let task_executor = ctx.task_executor;

        let max_tasks = determine_max_tasks(self.max_tasks);
        init_thread_pools(max_tasks as usize);

        let (metrics_tx, metrics_rx) = unbounded_channel();
        let metrics_listener = ParserMetricsListener::new(UnboundedYapperReceiver::new(
            metrics_rx,
            10_000,
            "metrics".to_string(),
        ));

        task_executor.spawn_critical("metrics", metrics_listener);

        let hr = self.try_start_fallback_server().await;

        tracing::info!(target: "brontes", "starting database initialization at: '{}'", brontes_db_path);
        let libmdbx =
            static_object(load_database(&task_executor, brontes_db_path, hr, self.run_id).await?);

        let tip = static_object(load_tip_database(libmdbx)?);
        tracing::info!(target: "brontes", "initialized libmdbx database");

        let load_window = self.load_time_window();

        let cex_download_config = CexDownloadConfig::new(
            // the run time window. notably we download the max window
            (load_window as u64, load_window as u64),
            self.cex_exchanges.clone(),
        );

        let range_type = self.get_range_type()?;
        let clickhouse = static_object(load_clickhouse(cex_download_config, self.run_id).await?);
        tracing::info!(target: "brontes", "Databases initialized");

        let only_cex_dex = self
            .inspectors
            .as_ref()
            .map(|f| {
                f.len() == 1
                    && (f.contains(&Inspectors::CexDex) || f.contains(&Inspectors::CexDexMarkout))
            })
            .unwrap_or(false);

        if only_cex_dex {
            self.force_no_dex_pricing = true;
        }

        let trade_config = self.time_window_args.trade_config();

        let inspectors = init_inspectors(
            quote_asset,
            libmdbx,
            self.inspectors,
            self.cex_exchanges,
            trade_config,
            self.with_metrics,
        );

        let tracer =
            get_tracing_provider(Path::new(&reth_db_path), max_tasks, task_executor.clone());
        let parser = static_object(DParser::new(metrics_tx, libmdbx, tracer.clone()).await);

        let executor = task_executor.clone();
        let result = executor
            .clone()
            .spawn_critical_with_graceful_shutdown_signal("run init", |shutdown| async move {
                if let Ok(brontes) = BrontesRunConfig::<_, _, _, MevProcessor>::new(
                    range_type,
                    max_tasks,
                    self.min_batch_size,
                    quote_asset,
                    self.force_dex_pricing,
                    self.force_no_dex_pricing,
                    inspectors,
                    clickhouse,
                    parser,
                    libmdbx,
                    tip,
                    self.cli_only,
                    self.with_metrics,
                    snapshot_mode,
                    load_window,
                    self.max_pending,
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

    pub fn get_range_type(&self) -> eyre::Result<RangeType> {
        if let Some(ranges) = &self.ranges {
            let parsed_ranges = parse_ranges(ranges).map_err(|e| eyre::eyre!(e))?;
            Ok(RangeType::MultipleRanges(parsed_ranges))
        } else {
            Ok(RangeType::SingleRange {
                start_block:   self.start_block,
                end_block:     self.end_block,
                back_from_tip: self.behind_tip,
                from_db_tip:   self.from_db_tip,
            })
        }
    }

    async fn try_start_fallback_server(&self) -> Option<HeartRateMonitor> {
        if self.enable_fallback {
            if let Some(fallback_server) = self.fallback_server.clone() {
                tracing::info!("starting heartbeat");
                backup_server_heartbeat(fallback_server, Duration::from_secs(4)).await;
                None
            } else {
                tracing::info!("starting monitor");
                let (tx, rx) = tokio::sync::mpsc::channel(10);
                if let Err(e) = start_hr_monitor(tx).await {
                    tracing::error!(err=%e);
                }
                tracing::info!("monitor server started");
                Some(HeartRateMonitor::new(Duration::from_secs(7), rx))
            }
        } else {
            None
        }
    }

    /// the time window in seconds for downloading
    fn load_time_window(&self) -> usize {
        self.time_window_args
            .max_vwap_pre
            .max(self.time_window_args.max_vwap_post)
            .max(self.time_window_args.max_optimistic_pre)
            .max(self.time_window_args.max_optimistic_post) as usize
    }

    fn check_proper_range(&self) -> eyre::Result<()> {
        if let (Some(start), Some(end)) = (&self.start_block, &self.end_block) {
            if start > end {
                return Err(eyre::eyre!("start block must be less than end block"))
            }
        }
        Ok(())
    }
}

fn parse_ranges(ranges: &[String]) -> Result<Vec<(u64, u64)>, String> {
    ranges
        .iter()
        .map(|range| {
            let (start, end) = range
                .split_once('-')
                .ok_or_else(|| format!("invalid range: {}", range))?;
            let start: u64 = start
                .parse()
                .map_err(|_| format!("invalid start block: {}", start))?;
            let end: u64 = end
                .parse()
                .map_err(|_| format!("invalid end block: {}", end))?;
            if start > end {
                return Err(format!(
                    "start block {} must be less than or equal to end block {}",
                    start, end
                ))
            }
            Ok((start, end))
        })
        .collect()
}

#[derive(Debug, Parser)]
pub struct TimeWindowArgs {
    /// The initial sliding time window (BEFORE) for cex prices or trades
    /// relative to the block timestamp
    #[arg(long = "initial-pre", default_value = "0.05")]
    pub initial_vwap_pre: f64,

    /// The initial sliding time window (AFTER) for cex prices or trades
    /// relative to the block timestamp
    #[arg(long = "initial-post", default_value = "0.05")]
    pub initial_vwap_post: f64,

    /// The maximum sliding time window (BEFORE) for cex prices or trades
    /// relative to the block timestamp
    #[arg(long = "max-vwap-pre", short = 'b', default_value = "10.0")]
    pub max_vwap_pre: f64,

    /// The maximum sliding time window (AFTER) for cex prices or trades
    /// relative to the block timestamp
    #[arg(long = "max-vwap-post", short = 'a', default_value = "20.0")]
    pub max_vwap_post: f64,

    /// Defines how much to extend the post-block time window before the
    /// pre-block.
    #[arg(long = "vwap-scaling-diff", default_value = "0.3")]
    pub vwap_scaling_diff: f64,

    /// Size of each extension to the vwap calculations time window
    #[arg(long = "vwap-time-step", default_value = "0.01")]
    pub vwap_time_step: f64,

    /// Use block time weights to favour prices closer to the block time
    #[arg(long = "weights-vwap", default_value = "true")]
    pub block_time_weights_vwap: bool,

    /// Rate of decay of bi-exponential decay function see calculate_weight in
    /// brontes_types::db::cex
    #[arg(long = "weights-pre-vwap", default_value = "-0.0000005")]
    pub pre_decay_weight_vwap: f64,

    /// Rate of decay of bi-exponential decay function see calculate_weight in
    /// brontes_types::db::ce
    #[arg(long = "weights-post-vwap", default_value = "-0.0000002")]
    pub post_decay_weight_vwap: f64,

    /// The initial time window (BEFORE) for cex prices or trades relative to
    /// the block timestamp for fully optimistic calculations
    #[arg(long = "initial-op-pre", default_value = "0.05")]
    pub initial_optimistic_pre: f64,

    /// The initial time window (AFTER) for cex prices or trades relative to the
    /// block timestamp for fully optimistic calculations
    #[arg(long = "initial-op-post", default_value = "0.3")]
    pub initial_optimistic_post: f64,

    /// The maximum time window (BEFORE) for cex prices or trades relative to
    /// the block timestamp for fully optimistic calculations
    #[arg(long = "max-op-pre", default_value = "5.0")]
    pub max_optimistic_pre: f64,

    /// The maximum time window (AFTER) for cex prices or trades relative to the
    /// block timestamp for fully optimistic calculations
    #[arg(long = "max-op-post", default_value = "10.0")]
    pub max_optimistic_post: f64,

    /// Defines how much to extend the post-block time window before the
    /// pre-block.
    #[arg(long = "optimistic-scaling-diff", default_value = "0.2")]
    pub optimistic_scaling_diff: f64,

    /// Size of each extension to the optimistic calculations time window
    #[arg(long = "optimistic-time-step", default_value = "0.1")]
    pub optimistic_time_step: f64,

    /// Use block time weights to favour prices closer to the block time
    #[arg(long = "weights-op", default_value = "true")]
    pub block_time_weights_optimistic: bool,

    /// Rate of decay of bi-exponential decay function see calculate_weight in
    /// brontes_types::db::cex
    #[arg(long = "weights-pre-op", default_value = "-0.0000003")]
    pub pre_decay_weight_optimistic: f64,

    /// Rate of decay of bi-exponential decay function see calculate_weight in
    /// brontes_types::db::ce
    #[arg(long = "weights-post-op", default_value = "-0.00000012")]
    pub post_decay_weight_optimistic: f64,

    /// Cex Dex Quotes price time offset from block timestamp
    #[arg(long = "quote-offset", default_value = "0.0")]
    pub quote_offset: f64,
}

impl TimeWindowArgs {
    fn trade_config(&self) -> CexDexTradeConfig {
        CexDexTradeConfig {
            initial_vwap_pre_block_us:  (self.initial_vwap_pre * SECONDS_TO_US_FLOAT) as u64,
            initial_vwap_post_block_us: (self.initial_vwap_post * SECONDS_TO_US_FLOAT) as u64,
            max_vwap_pre_block_us:      (self.max_vwap_pre * SECONDS_TO_US_FLOAT) as u64,
            max_vwap_post_block_us:     (self.max_vwap_post * SECONDS_TO_US_FLOAT) as u64,
            vwap_scaling_diff_us:       (self.vwap_scaling_diff * SECONDS_TO_US_FLOAT) as u64,
            vwap_time_step_us:          (self.vwap_time_step * SECONDS_TO_US_FLOAT) as u64,

            use_block_time_weights_vwap:       self.block_time_weights_vwap,
            pre_decay_weight_vwap:             self.pre_decay_weight_vwap,
            post_decay_weight_vwap:            self.post_decay_weight_vwap,
            initial_optimistic_pre_block_us:   (self.initial_optimistic_pre * SECONDS_TO_US_FLOAT)
                as u64,
            initial_optimistic_post_block_us:  (self.initial_optimistic_post * SECONDS_TO_US_FLOAT)
                as u64,
            max_optimistic_pre_block_us:       (self.max_optimistic_pre * SECONDS_TO_US_FLOAT)
                as u64,
            max_optimistic_post_block_us:      (self.max_optimistic_post * SECONDS_TO_US_FLOAT)
                as u64,
            optimistic_scaling_diff_us:        (self.optimistic_scaling_diff * SECONDS_TO_US_FLOAT)
                as u64,
            optimistic_time_step_us:           (self.optimistic_time_step * SECONDS_TO_US_FLOAT)
                as u64,
            use_block_time_weights_optimistic: self.block_time_weights_optimistic,
            pre_decay_weight_op:               self.pre_decay_weight_optimistic,
            post_decay_weight_op:              self.post_decay_weight_optimistic,
            quote_offset_from_block_us:        (self.quote_offset * SECONDS_TO_US_FLOAT) as u64,
        }
    }
}
