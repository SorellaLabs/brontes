use std::{path::Path, sync::Arc};

use brontes_database::{
    clickhouse::cex_config::CexDownloadConfig, libmdbx::initialize::LibmdbxInitializer,
};
use clap::Parser;
use indicatif::{ProgressBar, ProgressDrawTarget};
use tracing::{debug, error, info};

use crate::{
    cli::{get_tracing_provider, load_clickhouse, load_libmdbx, static_object},
    runner::CliContext,
};

/// downloads a range of data from clickhouse
#[derive(Debug, Parser)]
pub struct ClickhouseDownload {
    /// Start block
    #[arg(long, short)]
    pub start_block: u64,
    /// End block
    #[arg(long, short)]
    pub end_block:   u64,
    /// Table to download
    #[arg(short, long)]
    pub table:       brontes_database::Tables,
    /// Clear the table before downloading
    #[arg(short, long, default_value = "false")]
    pub clear_table: bool,
}

impl ClickhouseDownload {
    pub async fn execute(self, brontes_db_endpoint: String, ctx: CliContext) -> eyre::Result<()> {
        let table = self.table;
        info!(target: "brontes::db::clickhouse-download", "starting download for table: {:?}", table);
        let task = self.run(brontes_db_endpoint, ctx).await;

        if let Err(e) = task.as_ref() {
            error!(target: "brontes::db::clickhouse-download", "Error downloading data -- {:?}", e);
        }

        info!(target: "brontes::db::clickhouse-download", "finished download for table: {:?}", table);

        task?;

        Ok(())
    }

    async fn run(self, brontes_db_endpoint: String, ctx: CliContext) -> eyre::Result<()> {
        let libmdbx = static_object(load_libmdbx(&ctx.task_executor, brontes_db_endpoint.clone())?);
        debug!(target: "brontes::db::clickhouse-download", "made libmdbx");
        let cex_config = CexDownloadConfig::default();
        let clickhouse = static_object(load_clickhouse(cex_config, None).await?);
        debug!(target: "brontes::db::clickhouse-download", "made clickhouse");

        let tracer = Arc::new(get_tracing_provider(
            Path::new(&std::env::var("DB_PATH").expect("DB_PATH not found in .env")),
            10,
            ctx.task_executor.clone(),
        ));
        debug!(target: "brontes::db::clickhouse-download", "made tracer");

        let initializer = LibmdbxInitializer::new(libmdbx, clickhouse, tracer, true);

        let bar = ProgressBar::with_draw_target(
            Some(self.end_block - self.start_block),
            ProgressDrawTarget::stderr_with_hz(100),
        );

        let pre = std::time::Instant::now();
        initializer
            .initialize(
                self.table,
                self.clear_table,
                Some((self.start_block, self.end_block)),
                Arc::new(vec![(self.table, bar)]),
            )
            .await?;

        let time_taken = std::time::Instant::now().duration_since(pre);
        info!(target: "brontes::db::clickhouse-download", "Table: {:?} -- Time Elapsed {}", self.table, time_taken.as_secs());

        Ok(())
    }
}
