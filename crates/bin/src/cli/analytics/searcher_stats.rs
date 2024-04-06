use brontes_analytics::BrontesAnalytics;
use brontes_metrics::PoirotMetricsListener;
use clap::Parser;
use tokio::sync::mpsc::unbounded_channel;

use crate::{
    cli::{determine_max_tasks, init_brontes_db, init_tracer},
    runner::CliContext,
};

//TODO: Convert to notebooks searcher + builder profit stats
#[derive(Debug, Parser)]
pub struct GetStats {
    #[arg(long)]
    pub max_tasks: Option<u64>,
}

impl GetStats {
    pub async fn execute(self, ctx: CliContext) -> eyre::Result<()> {
        let libmdbx = init_brontes_db()?;

        let task_executor = ctx.task_executor;

        let (_metrics_tx, metrics_rx) = unbounded_channel();
        let metrics_listener = PoirotMetricsListener::new(metrics_rx);
        task_executor.spawn_critical("metrics", metrics_listener);

        let max_tasks = determine_max_tasks(self.max_tasks);
        let tracer = init_tracer(task_executor.clone(), max_tasks)?;

        let brontes_analytics = BrontesAnalytics::new(libmdbx, tracer.clone(), None);

        let _ = brontes_analytics.get_searcher_stats_by_mev_type().await;

        Ok(())
    }
}
