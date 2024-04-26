use brontes_types::db::traits::{DBWriter, LibmdbxReader};
use clap::Parser;
use itertools::Itertools;

use crate::{
    cli::utils::{load_database, static_object},
    runner::CliContext,
};
#[derive(Debug, Parser)]
pub struct LMem {
    #[arg(long, short)]
    pub start: u64,
    #[arg(long, short)]
    pub end:   u64,
}

impl LMem {
    pub async fn execute(self, brontes_db_endpoint: String, ctx: CliContext) -> eyre::Result<()> {
        let libmdbx = static_object(load_database(&ctx.task_executor, brontes_db_endpoint)?);

        let mut set = vec![];
        for block_range in (self.start..self.end)
            .chunks(100_000)
            .into_iter()
            .map(|f| f.collect_vec())
        {
            set.push(
                ctx.task_executor
                    .spawn_critical_blocking("test_mem", async move {
                        let mut cnt = 0usize;
                        for block in block_range {
                            if let Ok(t) = libmdbx.load_trace(block) {
                                cnt += 1;
                                let _ = libmdbx.save_traces(block, t).await;
                            }

                            if let Ok(d) = libmdbx.get_dex_quotes(block) {
                                cnt += 1;
                                let _ = libmdbx.write_dex_quotes(block, Some(d)).await;
                            }
                            cnt += libmdbx.get_metadata(block).is_ok() as usize;
                        }
                        println!("{cnt}");
                    }),
            );
        }

        for s in set {
            s.await?;
        }

        Ok(())
    }
}
