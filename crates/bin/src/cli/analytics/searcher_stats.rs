use std::{env, path::Path};

use brontes_analytics::BrontesAnalytics;
use brontes_metrics::PoirotMetricsListener;
use brontes_types::{db::searcher::Fund, mev::bundle::MevType, Protocol};
use clap::Parser;
use tokio::sync::mpsc::unbounded_channel;

use crate::{
    cli::{determine_max_tasks, get_env_vars, get_tracing_provider, load_database, static_object},
    runner::CliContext,
};

//TODO: Convert to notebooks searcher + builder profit stats
#[derive(Debug, Parser)]
pub struct GetStats {
    /// Start Block
    #[arg(long, short)]
    pub start_block: u64,
    /// Optional End Block, if omitted it will continue to run until killed
    #[arg(long, short)]
    pub end_block:   u64,
    /// Optional protocols to filter searcher bundles by
    #[arg(long, short, value_delimiter = ',')]
    pub protocols:   Option<Vec<Protocol>>,
    /// Optional fund filter
    #[arg(long, short, value_delimiter = ',')]
    pub funds:       Option<Vec<Fund>>,
    /// Optional MevType to filter searcher bundles by
    #[arg(long, short, value_delimiter = ',')]
    pub mev_types:   Option<Vec<MevType>>,
    /// Optional Max Tasks, if omitted it will default to 80% of the number of
    /// physical cores on your machine
    #[arg(long, short)]
    pub max_tasks:   Option<u64>,
}

impl GetStats {
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

        let _ = brontes_analytics
            .get_searcher_stats(
                self.start_block,
                self.end_block,
                self.mev_types,
                self.protocols,
                self.funds,
            )
            .await;

        Ok(())
    }
}
