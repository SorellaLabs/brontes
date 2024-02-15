use std::{env, path::Path};

use brontes_analytics::BrontesAnalytics;
use brontes_database::{
    libmdbx::{cursor::CompressedCursor, Libmdbx, LibmdbxReadWriter},
    CompressedTable, IntoTableKey, Tables,
};
use clap::Parser;
use itertools::Itertools;
use reth_db::mdbx::RO;
use reth_interfaces::db::DatabaseErrorInfo;

use super::{determine_max_tasks, get_env_vars, get_tracing_provider, static_object};
use crate::runner::CliContext;

#[derive(Debug, Parser)]
pub struct Analytics {
    /// Start Block
    #[arg(long, short)]
    pub start_block: u64,
    /// Optional End Block, if omitted it will continue to run until killed
    #[arg(long, short)]
    pub end_block: Option<u64>,
    /// Optional Max Tasks, if omitted it will default to 80% of the number of
    /// physical cores on your machine
    #[arg(long, short)]
    pub max_tasks: Option<u64>,
}

impl Analytics {
    pub async fn execute(self, ctx: CliContext) -> eyre::Result<()> {
        let db_path = get_env_vars()?;

        let brontes_db_endpoint = env::var("BRONTES_DB_PATH").expect("No BRONTES_DB_PATH in .env");
        let libmdbx = static_object(LibmdbxReadWriter::init_db(brontes_db_endpoint, None)?);
        let task_executor = ctx.task_executor;

        let max_tasks = determine_max_tasks(self.max_tasks);
        let tracer = static_object(get_tracing_provider(
            Path::new(&db_path),
            max_tasks,
            task_executor.clone(),
        ));

        let brontes_analytics = BrontesAnalytics::new(libmdbx, tracer);

        brontes_analytics
            .get_vertically_integrated_searchers(
                self.start_block,
                self.end_block.unwrap_or(u64::MAX),
            )
            .await?;

        Ok(())
    }
}
