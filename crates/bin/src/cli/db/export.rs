use std::env;

use brontes_database::parquet::ParquetExporter;
use clap::Parser;

use crate::{
    cli::{load_libmdbx, static_object},
    runner::CliContext,
};

#[derive(Debug, Parser)]
pub struct Export {
    /// that table to query
    //#[arg(long, short)]
    //pub table: Tables,
    /// Start Block
    #[arg(long, short)]
    pub start_block: u64,
    /// Optional End Block, if omitted it will export until last entry
    #[arg(long, short)]
    pub end_block:   u64,
}

impl Export {
    pub async fn execute(self, _ctx: CliContext) -> eyre::Result<()> {
        let brontes_db_endpoint = env::var("BRONTES_DB_PATH").expect("No BRONTES_DB_PATH in .env");
        let libmdbx = static_object(load_libmdbx(brontes_db_endpoint)?);

        let exporter = ParquetExporter::new(self.start_block, self.end_block, libmdbx);

        exporter
            .export_mev_blocks_and_bundles()
            .await
            .expect("Failed to export mev data to parquet");

        Ok(())
    }
}
