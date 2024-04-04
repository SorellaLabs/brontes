use std::{env, path::Path};

use brontes_core::decoding::Parser as DParser;
use brontes_metrics::PoirotMetricsListener;
use brontes_types::{init_threadpools, unordered_buffer_map::BrontesStreamExt};
use clap::Parser;
use futures::StreamExt;
use tokio::sync::mpsc::unbounded_channel;

use crate::{
    cli::{determine_max_tasks, get_db_path, get_tracing_provider, load_database, static_object},
    runner::CliContext,
};

#[derive(Debug, Parser)]
pub struct TestTraceArgs {
    #[arg(long, short, value_delimiter = ',')]
    pub blocks: Vec<u64>,
}

impl TestTraceArgs {
    pub async fn execute(self, ctx: CliContext) -> eyre::Result<()> {
        let db_path = get_db_path()?;

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

        futures::stream::iter(self.blocks.into_iter())
            .unordered_buffer_map(100, |i| parser.execute(i))
            .map(|_res| ())
            .collect::<Vec<_>>()
            .await;

        Ok(())
    }
}
