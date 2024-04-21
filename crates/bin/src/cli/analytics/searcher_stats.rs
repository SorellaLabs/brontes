use std::path::Path;

use brontes_analytics::BrontesAnalytics;
use brontes_metrics::PoirotMetricsListener;
use clap::Parser;
use tokio::sync::mpsc::unbounded_channel;

use crate::{
    cli::{determine_max_tasks, get_env_vars, get_tracing_provider, load_database, static_object},
    runner::CliContext,
};

//TODO: Convert to notebooks searcher + builder profit stats
#[derive(Debug, Parser)]
pub struct GetStats {
    #[arg(long)]
    pub max_tasks: Option<u64>,
}

impl GetStats {
    pub async fn execute(self, brontes_db_endpoint: String, ctx: CliContext) -> eyre::Result<()> {
        let db_path = get_env_vars()?;

        let libmdbx = static_object(load_database(&ctx.task_executor, brontes_db_endpoint)?);

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

        let brontes_analytics = BrontesAnalytics::new(libmdbx, tracer.clone(), None);

        let _ = brontes_analytics.get_searcher_stats_by_mev_type().await;

        Ok(())
    }
}
