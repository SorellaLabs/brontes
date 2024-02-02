use std::{env, path::Path};

use brontes_core::decoding::Parser as DParser;
use brontes_database::{
    clickhouse::Clickhouse,
    libmdbx::{LibmdbxReadWriter, LibmdbxReader},
};
use brontes_metrics::PoirotMetricsListener;
use brontes_types::unordered_buffer_map::BrontesStreamExt;
use clap::Parser;
use futures::{stream::FuturesUnordered, StreamExt};
use itertools::Itertools;
use tokio::sync::mpsc::unbounded_channel;

use super::{determine_max_tasks, get_env_vars, static_object};
use crate::{cli::get_tracing_provider, runner::CliContext};
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
        let db_path = get_env_vars()?;

        let max_tasks = determine_max_tasks(None) * 2;
        let (metrics_tx, metrics_rx) = unbounded_channel();

        let metrics_listener = PoirotMetricsListener::new(metrics_rx);
        ctx.task_executor
            .spawn_critical("metrics", metrics_listener);

        let brontes_db_endpoint = env::var("BRONTES_DB_PATH").expect(
            "No
        BRONTES_DB_PATH in .env",
        );

        let libmdbx = static_object(LibmdbxReadWriter::init_db(brontes_db_endpoint, None)?);
        let _clickhouse = static_object(Clickhouse::default());

        let tracer =
            get_tracing_provider(&Path::new(&db_path), max_tasks, ctx.task_executor.clone());

        let parser = static_object(DParser::new(
            metrics_tx,
            libmdbx,
            tracer.clone(),
            Box::new(|address, db_tx| db_tx.get_protocol(*address).unwrap().is_none()),
        ));

        let amount = (self.end_block - self.start_block) as f64;

        futures::stream::iter(self.start_block..self.end_block)
            .unordered_buffer_map(10_000, |i| {
                if i % 5000 == 0 {
                    tracing::info!(
                        "tracing {:.2}% done",
                        (i - self.start_block) as f64 / amount * 100.0
                    );
                }
                parser.execute(i)
            })
            .map(|res| ())
            .collect::<Vec<_>>()
            .await;

        Ok(())
    }
}
