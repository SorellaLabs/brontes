#[allow(unused_imports)]
use std::path::Path;

#[allow(unused_imports)]
use brontes_database::Tables;
#[allow(unused_imports)]
use brontes_metrics::PoirotMetricsListener;
#[allow(unused_imports)]
use brontes_types::{init_thread_pools, UnboundedYapperReceiver};
#[allow(unused_imports)]
use clap::Parser;
#[allow(unused_imports)]
use tokio::sync::mpsc::unbounded_channel;

#[allow(unused_imports)]
use crate::{
    cli::{determine_max_tasks, get_env_vars, get_tracing_provider, load_database, static_object},
    runner::CliContext,
};

#[derive(Debug, Parser)]
pub struct Backfill {
    /// Start Block
    #[arg(long, short)]
    pub start_block: u64,
    /// block to trace to
    #[arg(long, short)]
    pub end_block:   u64,
    /// Table to backfill
    #[arg(long, short)]
    pub table:       Tables,
    /// Max tasks to run
    #[arg(long, short)]
    pub max_tasks:   Option<u64>,
}

impl Backfill {
    pub async fn execute(self, _brontes_db_endpoint: String, ctx: CliContext) -> eyre::Result<()> {
        let _db_path = get_env_vars()?;

        let max_tasks = determine_max_tasks(self.max_tasks);
        init_thread_pools(max_tasks as usize);
        let (_metrics_tx, metrics_rx) = unbounded_channel();

        let metrics_listener = PoirotMetricsListener::new(UnboundedYapperReceiver::new(
            metrics_rx,
            10_000,
            "metrics".to_string(),
        ));

        ctx.task_executor
            .spawn_critical("metrics", metrics_listener);
        /*
        let libmdbx = static_object(
            load_database(&ctx.task_executor, brontes_db_endpoint, None, None).await?,
        );

        let tracer =
            get_tracing_provider(Path::new(&db_path), max_tasks, ctx.task_executor.clone());

        let parser = static_object(DParser::new(metrics_tx, libmdbx, tracer.clone()).await);

        let amount = (self.end_block - self.start_block) as f64;
        */
        Ok(())
    }
}
