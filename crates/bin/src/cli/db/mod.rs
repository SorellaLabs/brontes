use clap::{Parser, Subcommand};

mod libmdbx_mem;
use crate::runner::CliContext;
mod db_clear;
mod db_insert;
mod db_query;
#[cfg(feature = "local-clickhouse")]
mod ensure_test_traces;
mod export;
mod init;
#[cfg(all(feature = "local-clickhouse", not(feature = "local-no-inserts")))]
mod tip_tracer;
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
    DbInserts(db_insert::Insert),
    /// Query data from any libmdbx table and pretty print it in stdout
    #[command(name = "query")]
    DbQuery(db_query::DatabaseQuery),
    /// Clear a libmdbx table
    #[command(name = "clear")]
    DbClear(db_clear::Clear),
    /// Generates traces and will store them in libmdbx (also clickhouse if
    /// --feature local-clickhouse)
    #[command(name = "generate-traces")]
    TraceRange(trace_range::TraceArgs),
    #[command(name = "libmdbx-mem-test")]
    LibmdbxMem(libmdbx_mem::LMem),
    /// For a given range, will fetch all data from the api and insert it into
    /// libmdbx.
    #[command(name = "init")]
    Init(init::Init),
    #[command(name = "export")]
    Export(export::Export),
    /// Traces all blocks needed for testing and inserts them into
    /// clickhouse
    #[cfg(feature = "local-clickhouse")]
    #[command(name = "test-traces-init")]
    TestTracesInit(ensure_test_traces::TestTraceArgs),
    #[cfg(all(feature = "local-clickhouse", not(feature = "local-no-inserts")))]
    #[command(name = "trace-at-tip")]
    TraceAtTip(tip_tracer::TipTraceArgs),
}

impl Database {
    pub async fn execute(self, brontes_db_endpoint: String, ctx: CliContext) -> eyre::Result<()> {
        match self.command {
            DatabaseCommands::DbInserts(cmd) => cmd.execute(brontes_db_endpoint).await,
            DatabaseCommands::DbQuery(cmd) => cmd.execute(brontes_db_endpoint).await,
            DatabaseCommands::TraceRange(cmd) => cmd.execute(brontes_db_endpoint, ctx).await,
            DatabaseCommands::Init(cmd) => cmd.execute(brontes_db_endpoint, ctx).await,
            DatabaseCommands::DbClear(cmd) => cmd.execute(brontes_db_endpoint).await,
            DatabaseCommands::LibmdbxMem(cmd) => cmd.execute(brontes_db_endpoint, ctx).await,
            DatabaseCommands::Export(cmd) => cmd.execute(brontes_db_endpoint, ctx).await,
            #[cfg(feature = "local-clickhouse")]
            DatabaseCommands::TestTracesInit(cmd) => cmd.execute(brontes_db_endpoint, ctx).await,
            #[cfg(all(feature = "local-clickhouse", not(feature = "local-no-inserts")))]
            DatabaseCommands::TraceAtTip(cmd) => cmd.execute(brontes_db_endpoint, ctx).await,
        }
    }
}
