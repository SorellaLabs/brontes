use std::{env, path::Path};

use brontes_core::decoding::Parser as DParser;
use brontes_metrics::PoirotMetricsListener;
use brontes_types::{init_threadpools, unordered_buffer_map::BrontesStreamExt};
use clap::Parser;
use futures::{join, StreamExt};
use tokio::sync::mpsc::unbounded_channel;

use crate::{
    cli::{determine_max_tasks, get_env_vars, get_tracing_provider, load_database, static_object},
    runner::CliContext,
};

#[derive(Debug, Parser)]
pub struct TipTraceArgs {
    /// Start Block
    #[arg(long, short)]
    pub start_block: u64,
}

impl TipTraceArgs {
    pub async fn execute(self, ctx: CliContext) -> eyre::Result<()> {
        let db_path = get_env_vars()?;

        let max_tasks = determine_max_tasks(None) * 2;
        init_threadpools(max_tasks as usize);
        let (metrics_tx, metrics_rx) = unbounded_channel();

        let metrics_listener = PoirotMetricsListener::new(metrics_rx);
        ctx.task_executor
            .spawn_critical("metrics", metrics_listener);

        let brontes_db_endpoint = env::var("BRONTES_DB_PATH").expect(
            "No
        BRONTES_DB_PATH in .env",
        );

        let libmdbx = static_object(load_database(brontes_db_endpoint)?);

        let tracer =
            get_tracing_provider(Path::new(&db_path), max_tasks, ctx.task_executor.clone());

        let parser = static_object(DParser::new(metrics_tx, libmdbx, tracer.clone()).await);
        let mut end_block = parser.get_latest_block_number().unwrap();

        // trace up to chaintip
        let catchup = ctx.task_executor.spawn_critical("catchup", async move {
            futures::stream::iter(self.start_block..=end_block)
                .unordered_buffer_map(100, |i| parser.execute(i))
                .map(|_res| ())
                .collect::<Vec<_>>()
                .await;
        });

        let tip = ctx.task_executor.spawn_critical("tip", async move {
            loop {
                let tip = parser.get_latest_block_number().unwrap();
                if tip + 1 > end_block {
                    end_block += 1;
                    let _ = parser.execute(end_block).await;
                }
            }
        });

        ctx.task_executor
            .spawn_critical("tasks", async move {
                let _ = join!(catchup, tip);
            })
            .await?;

        Ok(())
    }
}
