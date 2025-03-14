use std::sync::Arc;

use brontes_database::{parquet::ParquetExporter, Tables};
use clap::Parser;
use futures::future::join_all;
use tokio::task::spawn;
use tracing::error;

use crate::{
    cli::{load_libmdbx, static_object},
    runner::CliContext,
};
#[derive(Debug, Parser)]
pub struct Export {
    /// Optional tables to exports, if omitted will export all supported tables
    #[arg(long, short, default_values = &["MevBlocks", "AddressMeta", "SearcherContracts", "Builder"], value_delimiter = ',', ignore_case=true)]
    pub tables:      Vec<Tables>,
    /// Optional Start Block, if omitted it will export the entire range to
    /// parquet
    #[arg(long, short)]
    pub start_block: Option<u64>,
    /// Optional End Block
    #[arg(long, short)]
    pub end_block:   Option<u64>,
    /// Optional path, will default to "data_exports/"
    #[arg(long, short)]
    pub path:        Option<String>,
}

impl Export {
    pub async fn execute(self, brontes_db_path: String, ctx: CliContext) -> eyre::Result<()> {
        let libmdbx = static_object(load_libmdbx(&ctx.task_executor, brontes_db_path)?);
        let exporter =
            Arc::new(ParquetExporter::new(self.start_block, self.end_block, self.path, libmdbx));

        let futures = self.tables.into_iter().map(|t| {
            let exporter = exporter.clone();
            spawn(async move { t.export_to_parquet(exporter).await })
        });

        let results = join_all(futures).await;

        for result in results {
            match result {
                Ok(Ok(_)) => (),
                Ok(Err(e)) => {
                    error!("Failed to export table: {}", e);
                    return Err(e);
                }
                Err(e) => {
                    error!("Task failed: {}", e);
                    return Err(eyre::eyre!("Task failed: {}", e));
                }
            }
        }

        Ok(())
    }
}
