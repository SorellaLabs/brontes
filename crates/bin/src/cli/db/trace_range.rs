

use brontes_core::decoding::Parser as DParser;

use brontes_types::{init_threadpools, unordered_buffer_map::BrontesStreamExt};
use clap::Parser;
use futures::StreamExt;


use crate::{
    cli::{
        determine_max_tasks, init_brontes_db, init_metrics_listener,
        init_tracer, static_object,
    },
    runner::CliContext,
};

#[derive(Debug, Parser)]
pub struct TraceArgs {
    /// Start Block
    #[arg(long, short)]
    pub start_block: u64,
    /// block to trace to
    #[arg(long, short)]
    pub end_block:   u64,
}

impl TraceArgs {
    pub async fn execute(self, ctx: CliContext) -> eyre::Result<()> {
        let max_tasks = determine_max_tasks(None) * 2;
        init_threadpools(max_tasks as usize);

        let metrics_tx = init_metrics_listener(&ctx.task_executor);

        let libmdbx = init_brontes_db()?;

        let tracer = init_tracer(ctx.task_executor, max_tasks)?;

        let parser = static_object(DParser::new(metrics_tx, libmdbx, tracer.clone()).await);

        let amount = (self.end_block - self.start_block) as f64;

        futures::stream::iter(self.start_block..self.end_block)
            .unordered_buffer_map(100, |i| {
                if i % 5000 == 0 {
                    tracing::info!(
                        "tracing {:.2}% done",
                        (i - self.start_block) as f64 / amount * 100.0
                    );
                }
                parser.execute(i)
            })
            .map(|_res| ())
            .collect::<Vec<_>>()
            .await;

        Ok(())
    }
}
