use std::{path::Path, sync::Arc};

use brontes_core::LibmdbxReadWriter;
use brontes_database::{
    clickhouse::cex_config::CexDownloadConfig, libmdbx::initialize::LibmdbxInitializer, CexPrice,
};
use clap::Parser;
use indicatif::{ProgressBar, ProgressDrawTarget};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use tracing::{debug, error, info};

use crate::{
    cli::{get_tracing_provider, load_clickhouse, load_libmdbx, static_object},
    runner::CliContext,
};

/// checks for missing check data
#[derive(Debug, Parser)]
pub struct MissingCex {
    /// Start block
    #[arg(long, short)]
    pub start_block: u64,
    /// End block
    #[arg(long, short)]
    pub end_block:   u64,
    /// checks trades
    #[arg(short, long, default_value = "false")]
    pub trades:      bool,
    /// checks quotes
    #[arg(short, long, default_value = "false")]
    pub quotes:      bool,
}

impl MissingCex {
    pub fn execute(self, brontes_db_endpoint: String, ctx: CliContext) -> eyre::Result<()> {
        let task = self.run(brontes_db_endpoint, ctx);

        if let Err(e) = task.as_ref() {
            error!(target: "brontes::db::missing-cex", "error checking for missing cex data -- {:?}", e);
        }

        info!(target: "brontes::db::missing-cex", "finished checking for cex data");

        task?;

        Ok(())
    }

    fn run(self, brontes_db_endpoint: String, ctx: CliContext) -> eyre::Result<()> {
        let libmdbx = static_object(load_libmdbx(&ctx.task_executor, brontes_db_endpoint.clone())?);
        debug!(target: "brontes::db::missing-cex", "made libmdbx");


        (self.start_block ..= self.end_block).into_par_iter().map(|block_num| {
            if self.quotes {
                
            }
        })


        

        Ok(())
    }
    

    fn get_quotes(&self, db: &LibmdbxReadWriter) -> eyre::Result<Vec<u64>> {
        let block_counts = db.db.view_db(|txn| {

            let cursor = txn.cursor_read::<CexPrice>()?;

            cursor.walk_range(self.start_block..=self.end_block)?.map(|block_res| {
                let block = block_res?;
               let t= block.0;
            })


        })??;

        Ok(())
    }
}
