use clap::{Parser, Subcommand};

use crate::runner::CliContext;
mod db_insert;
mod db_query;
#[cfg(feature = "local-clickhouse")]
mod ensure_test_traces;
mod init;
mod trace_range;

#[derive(Debug, Parser)]
pub struct Database {
    #[clap(subcommand)]
    pub command: DatabaseCommands,
}

#[derive(Debug, Subcommand)]
pub enum DatabaseCommands {
    /// Allows for inserting items into libmdbx
    #[command(name = "insert")]
    DbInserts(db_insert::AddToDb),
    /// Query data from any libmdbx table and pretty print it in stdout
    #[command(name = "query")]
    DbQuery(db_query::DatabaseQuery),
    /// Generates traces and will store them in libmdbx (also clickhouse if
    /// --feature local-clickhouse)
    #[command(name = "generate-traces")]
    TraceRange(trace_range::TraceArgs),
    /// For a given range, will fetch all data from the api and insert it into
    /// libmdbx.
    #[command(name = "init")]
    Init(init::Init),
    /// Traces all blocks needed for testing and inserts them into
    /// clickhouse
    #[cfg(feature = "local-clickhouse")]
    #[command(name = "test-traces-init")]
    TestTracesInit(ensure_test_traces::TestTraceArgs),
}

impl Database {
    pub async fn execute(self, ctx: CliContext) -> eyre::Result<()> {
        match self.command {
            DatabaseCommands::DbInserts(cmd) => cmd.execute().await,
            DatabaseCommands::DbQuery(cmd) => cmd.execute().await,
            DatabaseCommands::TraceRange(cmd) => cmd.execute(ctx).await,
            DatabaseCommands::Init(cmd) => cmd.execute(ctx).await,
            #[cfg(feature = "local-clickhouse")]
            DatabaseCommands::TestTracesInit(cmd) => cmd.execute(ctx).await,
        }
    }
}
