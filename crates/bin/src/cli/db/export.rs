use std::env;

use brontes_core::LibmdbxReader;
use brontes_database::parquet::export_mev_blocks_and_bundles;
use brontes_types::init_threadpools;
use clap::Parser;

use crate::cli::{load_database, static_object};

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
    pub async fn execute(self) -> eyre::Result<()> {
        let brontes_db_endpoint = env::var("BRONTES_DB_PATH").expect("No BRONTES_DB_PATH in .env");
        init_threadpools(10);

        let libmdbx = static_object(load_database(brontes_db_endpoint)?);

        let mev_blocks = libmdbx.try_fetch_mev_blocks(self.start_block, self.end_block)?;

        export_mev_blocks_and_bundles(mev_blocks).await?;

        Ok(())
    }
}
