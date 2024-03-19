use std::env;

use brontes_database::{parquet::ParquetExporter, Tables};
use clap::Parser;
use futures::stream::{FuturesUnordered, StreamExt};
use tracing::error;

use crate::{
    cli::{load_libmdbx, static_object},
    runner::CliContext,
};

#[derive(Debug, Parser)]
pub struct Export {
    /// Optional tables to exports, if omitted will export all supported tables
    #[arg(long, short,default_values = &["MevBlocks", "AddressMeta"], value_delimiter = ',')]
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
    pub async fn execute(self, _ctx: CliContext) -> eyre::Result<()> {
        let brontes_db_endpoint = env::var("BRONTES_DB_PATH").expect("No BRONTES_DB_PATH in .env");
        let libmdbx = static_object(load_libmdbx(brontes_db_endpoint)?);

        let exporter = ParquetExporter::new(self.start_block, self.end_block, self.path, libmdbx);

        let mut futures = FuturesUnordered::new();

        for t in self.tables.iter() {
            futures.push(t.export_to_parquet(&exporter));
        }

        while let Some(result) = futures.next().await {
            if let Err(e) = result {
                error!("Failed to export table: {}", e);
                return Err(e);
            }
        }

        Ok(())
    }
}
