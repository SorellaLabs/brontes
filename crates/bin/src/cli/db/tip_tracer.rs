use std::path::Path;

use brontes_core::decoding::Parser as DParser;
use brontes_metrics::PoirotMetricsListener;
use brontes_types::{
    init_thread_pools, unordered_buffer_map::BrontesStreamExt, UnboundedYapperReceiver,
};
use clap::Parser;
use futures::{join, StreamExt};
use tokio::sync::mpsc::unbounded_channel;

use crate::{
    cli::{get_env_vars, get_tracing_provider, load_read_only_database, static_object},
    runner::CliContext,
};

#[derive(Debug, Parser)]
pub struct TipTraceArgs {
    /// Start Block
    #[arg(long, short)]
    pub start_block: Option<u64>,
}

impl TipTraceArgs {
    pub async fn execute(self, brontes_db_endpoint: String, ctx: CliContext) -> eyre::Result<()> {
        let db_path = get_env_vars()?;

        let max_tasks = (num_cpus::get_physical() as f64 * 0.7) as u64 + 1;
        init_thread_pools(max_tasks as usize);
        let (metrics_tx, metrics_rx) = unbounded_channel();

        let metrics_listener = PoirotMetricsListener::new(UnboundedYapperReceiver::new(
            metrics_rx,
            10_000,
            "metrics".to_string(),
        ));

        ctx.task_executor
            .spawn_critical("metrics", metrics_listener);

        let libmdbx =
            static_object(load_read_only_database(&ctx.task_executor, brontes_db_endpoint).await?);

        let tracer =
            get_tracing_provider(Path::new(&db_path), max_tasks, ctx.task_executor.clone());

        let parser = static_object(DParser::new(metrics_tx, libmdbx, tracer.clone()).await);
        let mut end_block = parser.get_latest_block_number().unwrap();

        let start_block = if let Some(s) = self.start_block {
            s
        } else {
            libmdbx.client.max_traced_block().await.unwrap()
        };

        // trace up to chaintip
        let catchup = ctx.task_executor.spawn_critical("catchup", async move {
            futures::stream::iter(start_block..=end_block)
                .unordered_buffer_map(100, |i| parser.trace_for_clickhouse(i))
                .map(|_| ())
                .collect::<Vec<_>>()
                .await;
        });

        let tip = ctx.task_executor.spawn_critical("tip", async move {
            loop {
                let tip = parser.get_latest_block_number().unwrap();
                if tip + 1 > end_block {
                    end_block += 1;
                    let _ = parser.trace_for_clickhouse(end_block).await;
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
