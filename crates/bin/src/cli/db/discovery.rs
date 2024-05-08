use std::path::Path;

use brontes_core::decoding::Parser as DParser;
use brontes_metrics::PoirotMetricsListener;
use brontes_types::{
    init_threadpools, unordered_buffer_map::BrontesStreamExt, UnboundedYapperReceiver,
};
use clap::Parser;
use futures::{join, StreamExt};
use indicatif::ProgressBar;
use itertools::Itertools;
use tokio::sync::mpsc::unbounded_channel;

use crate::{
    cli::{get_env_vars, get_tracing_provider, load_read_only_database, static_object},
    discovery_only::DiscoveryExecutor,
    runner::CliContext,
};

#[derive(Debug, Parser)]
pub struct DiscoveryFill {
    /// Start Block
    #[arg(long, short)]
    pub start_block: Option<u64>,
}

impl DiscoveryFill {
    pub async fn execute(self, brontes_db_endpoint: String, ctx: CliContext) -> eyre::Result<()> {
        let db_path = get_env_vars()?;

        let max_tasks = num_cpus::get_physical();
        init_threadpools(max_tasks as usize);

        let (metrics_tx, metrics_rx) = unbounded_channel();

        let metrics_listener = PoirotMetricsListener::new(UnboundedYapperReceiver::new(
            metrics_rx,
            10_000,
            "metrics".to_string(),
        ));

        ctx.task_executor
            .spawn_critical("metrics", metrics_listener);

        let libmdbx = static_object(load_read_only_database(brontes_db_endpoint)?);

        let tracer =
            get_tracing_provider(Path::new(&db_path), max_tasks as u64, ctx.task_executor.clone());

        let parser = static_object(DParser::new(metrics_tx, libmdbx, tracer.clone()).await);
        let mut end_block = parser.get_latest_block_number().unwrap();

        let start_block = if let Some(s) = self.start_block {
            s
        } else {
            libmdbx.client.max_traced_block().await.unwrap()
        };

        let end_block = parser.get_latest_block_number().unwrap();

        let bar = ProgressBar::new(end_block - start_block);

        let chunks = (start_block..=end_block)
            .chunks(max_tasks)
            .into_iter()
            .map(|mut c| {
                let start = c.next().unwrap();
                let end_block = c.last().unwrap_or(start_block);
                (start, end_block)
            })
            .collect_vec();

        futures::stream::iter(chunks)
            .map(|(start_block, end_block)| {
                ctx.task_executor
                    .spawn_critical_with_graceful_shutdown_signal(
                        "Discovery",
                        |shutdown| async move {
                            DiscoveryExecutor::new(
                                start_block,
                                end_block,
                                libmdbx,
                                parser,
                                bar.clone(),
                            )
                            .run_until_graceful_shutdown(shutdown)
                            .await
                        },
                    );
            })
            .collect::<Vec<_>>()
            .await;
        Ok(())
    }
}
