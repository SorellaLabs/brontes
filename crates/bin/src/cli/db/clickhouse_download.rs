use std::{path::Path, sync::Arc};

use brontes_database::{
    clickhouse::{cex_config::CexDownloadConfig, ClickhouseHandle},
    libmdbx::initialize::LibmdbxInitializer,
};
use brontes_types::traits::TracingProvider;
use clap::Parser;
use reth_tracing_ext::TracingClient;

use crate::{
    cli::{load_clickhouse, load_libmdbx, static_object},
    runner::CliContext,
};

/// downloads a range of data from clickhouse
#[derive(Debug, Parser)]
pub struct ClickhouseDownload {
    /// start block
    #[arg(long, short)]
    pub start_block: u64,
    /// end block
    #[arg(long, short)]
    pub end_block:   u64,
    /// table to download
    #[arg(short, long)]
    pub table:       brontes_database::Tables,
    /// clears the table before downloading
    #[arg(short, long, default_value = "false")]
    pub clear_table: bool,
}

impl ClickhouseDownload {
    pub async fn execute(self, brontes_db_endpoint: String, ctx: CliContext) -> eyre::Result<()> {
        let libmdbx = static_object(load_libmdbx(&ctx.task_executor, brontes_db_endpoint.clone())?);
        let cex_config = CexDownloadConfig::default();
        let clickhouse = static_object(load_clickhouse(cex_config).await?);

        let initializer = LibmdbxInitializer::new(
            libmdbx,
            clickhouse,
            Arc::new(TracingClient::new(
                &Path::new(&brontes_db_endpoint),
                10,
                ctx.task_executor.clone(),
            )),
        );

        ctx.task_executor
            .spawn_critical("download", {
                async move {
                    if let Err(e) = self.run(initializer).await {
                        eprintln!("Error downloading data -- {:?}", e);
                    }
                }
            })
            .await?;

        Ok(())
    }

    async fn run<TP: TracingProvider, CH: ClickhouseHandle>(
        self,
        initializer: LibmdbxInitializer<TP, CH>,
    ) -> eyre::Result<()> {
        let pre = std::time::Instant::now();
        initializer
            .initialize(
                self.table,
                self.clear_table,
                Some((self.start_block, self.end_block)),
                Arc::new(vec![]),
            )
            .await?;

        let time_taken = std::time::Instant::now().duration_since(pre);
        println!("Table: {:?} -- Time Elapsed {}", self.table, time_taken.as_secs());

        Ok(())
    }
}
