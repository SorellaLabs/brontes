use std::path::Path;

use brontes_core::decoding::Parser as DParser;
use brontes_metrics::PoirotMetricsListener;
use brontes_types::{init_thread_pools, UnboundedYapperReceiver};
use clap::Parser;
use futures::StreamExt;
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressState, ProgressStyle};
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
    /// Max number of tasks to run concurrently
    #[arg(long, short)]
    pub max_tasks:   Option<usize>,
}

impl DiscoveryFill {
    pub async fn execute(self, brontes_db_path: String, ctx: CliContext) -> eyre::Result<()> {
        let db_path = get_env_vars()?;

        let max_tasks = self.max_tasks.unwrap_or(num_cpus::get_physical());
        init_thread_pools(max_tasks);

        let (metrics_tx, metrics_rx) = unbounded_channel();

        let metrics_listener = PoirotMetricsListener::new(UnboundedYapperReceiver::new(
            metrics_rx,
            10_000,
            "metrics".to_string(),
        ));

        ctx.task_executor
            .spawn_critical("metrics", metrics_listener);

        let libmdbx =
            static_object(load_read_only_database(&ctx.task_executor, brontes_db_path).await?);

        let tracer =
            get_tracing_provider(Path::new(&db_path), max_tasks as u64, ctx.task_executor.clone());

        let parser = static_object(DParser::new(metrics_tx, libmdbx, tracer.clone()).await);

        let start_block = if let Some(s) = self.start_block {
            s
        } else {
            libmdbx.client.max_traced_block().await?
        };
        let end_block = parser.get_latest_block_number().unwrap();

        let bar = ProgressBar::with_draw_target(
            Some(end_block - start_block),
            ProgressDrawTarget::stderr_with_hz(100),
        );
        let style = ProgressStyle::default_bar()
            .template(
                "{msg}\n[{elapsed_precise}] [{wide_bar:.cyan/blue}] {pos}/{len} blocks \
                 ({percent}%) | ETA: {eta}",
            )
            .expect("Invalid progress bar template")
            .progress_chars("█>-")
            .with_key("eta", |state: &ProgressState, f: &mut dyn std::fmt::Write| {
                write!(f, "{:.1}s", state.eta().as_secs_f64()).unwrap()
            })
            .with_key("percent", |state: &ProgressState, f: &mut dyn std::fmt::Write| {
                write!(f, "{:.1}", state.fraction() * 100.0).unwrap()
            });
        bar.set_style(style);
        bar.set_message("Processing blocks:");

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
                let bar = bar.clone();
                ctx.task_executor
                    .spawn_critical_with_graceful_shutdown_signal(
                        "Discovery",
                        |shutdown| async move {
                            DiscoveryExecutor::new(start_block, end_block, libmdbx, parser, bar)
                                .run_until_graceful_shutdown(shutdown)
                                .await
                        },
                    )
            })
            .buffer_unordered(max_tasks)
            .collect::<Vec<_>>()
            .await;
        Ok(())
    }
}
