use std::path::Path;

use brontes_core::{decoding::Parser as DParser, LibmdbxReader};
use brontes_database::{
    clickhouse::cex_config::CexDownloadConfig,
    libmdbx::{cursor::CompressedCursor, Libmdbx},
    CompressedTable, IntoTableKey, Tables,
};
use brontes_inspect::Inspectors;
use brontes_metrics::PoirotMetricsListener;
use brontes_types::{constants::USDT_ADDRESS_STRING, db::cex::CexExchange, init_threadpools};
use clap::Parser;
use itertools::Itertools;
use reth_db::mdbx::RO;
use reth_interfaces::db::DatabaseErrorInfo;
use tokio::sync::mpsc::unbounded_channel;

use crate::{
    banner,
    cli::{
        get_tracing_provider, init_inspectors,
        utils::{determine_max_tasks, get_env_vars, load_clickhouse, load_database, static_object},
    },
    runner::CliContext,
    BrontesRunConfig, MevProcessor,
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
                            cnt += libmdbx.load_trace(block).is_ok() as usize;
                            cnt += libmdbx.get_dex_quotes(block).is_ok() as usize;
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
