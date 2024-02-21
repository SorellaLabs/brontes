use std::{env, path::Path};

use brontes_analytics::BrontesAnalytics;
use brontes_metrics::PoirotMetricsListener;
use brontes_types::mev::bundle::MevType;
use clap::{Parser, Subcommand};
use tokio::sync::mpsc::unbounded_channel;

use super::{determine_max_tasks, get_env_vars, get_tracing_provider, static_object};
use crate::{cli::load_database, runner::CliContext};

#[derive(Debug, Parser)]
pub struct Analytics {
    #[clap(subcommand)]
    pub command: AnalyticsCommands,
}

#[derive(Debug, Subcommand)]
pub enum AnalyticsCommands {
    /// Identifies vertically integrated searchers & maps them to their builders
    /// in the database
    #[command(name = "vertically-integrated-builders", alias = "vi-builders")]
    ViBuilders(SearcherBuilder),
}

#[derive(Debug, Parser)]
pub struct SearcherBuilder {
    /// Start Block
    #[arg(long, short)]
    pub start_block: u64,
    /// Optional End Block, if omitted it will continue to run until killed
    #[arg(long, short)]
    pub end_block:   Option<u64>,
    /// Optional Max Tasks, if omitted it will default to 80% of the number of
    /// physical cores on your machine
    #[arg(long, short)]
    pub max_tasks:   Option<u64>,
    /// Optional MevType to filter by (e.g. only CexDex bundles will be
    /// considered when identifying searcher to builder relationships)
    #[arg(long, short, value_delimiter = ',')]
    pub mev_type:    Option<Vec<MevType>>,
}

impl Analytics {
    pub async fn execute(self, ctx: CliContext) -> eyre::Result<()> {
        match self.command {
            AnalyticsCommands::ViBuilders(cmd) => cmd.execute(ctx).await,
        }
    }
}

impl SearcherBuilder {
    pub async fn execute(self, ctx: CliContext) -> eyre::Result<()> {
        let db_path = get_env_vars()?;

        let brontes_db_endpoint = env::var("BRONTES_DB_PATH").expect("No BRONTES_DB_PATH in .env");

        let libmdbx = static_object(load_database(brontes_db_endpoint)?);

        let task_executor = ctx.task_executor;

        let (_metrics_tx, metrics_rx) = unbounded_channel();
        let metrics_listener = PoirotMetricsListener::new(metrics_rx);
        task_executor.spawn_critical("metrics", metrics_listener);

        let max_tasks = determine_max_tasks(self.max_tasks);
        let tracer = static_object(get_tracing_provider(
            Path::new(&db_path),
            max_tasks,
            task_executor.clone(),
        ));

        let brontes_analytics = BrontesAnalytics::new(libmdbx, tracer.clone());

        brontes_analytics
            .get_vertically_integrated_searchers(
                self.start_block,
                self.end_block.unwrap_or(u64::MAX),
                self.mev_type,
            )
            .await?;

        Ok(())
    }
}
