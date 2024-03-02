use clap::{Parser, Subcommand};

use crate::runner::CliContext;
mod db_insert;
mod db_query;
mod init;
mod trace_range;

#[derive(Debug, Parser)]
pub struct Database {
    #[clap(subcommand)]
    pub command: DatabaseCommands,
}

#[derive(Debug, Subcommand)]
pub enum DatabaseCommands {
    /// Identifies vertically integrated searchers & maps them to their builders
    /// in the database
    #[command(name = "db-insert")]
    DbInserts(db_insert::AddToDb),
    #[command(name = "db-query")]
    DbQuery(db_query::DatabaseQuery),
    #[command(name = "generate-traces")]
    TraceRange(trace_range::TraceArgs),
    #[command(name = "init")]
    Init(init::Init),
}

impl Database {
    pub async fn execute(self, ctx: CliContext) -> eyre::Result<()> {
        match self.command {
            DatabaseCommands::DbInserts(cmd) => cmd.execute().await,
            DatabaseCommands::DbQuery(cmd) => cmd.execute().await,
            DatabaseCommands::TraceRange(cmd) => cmd.execute(ctx).await,
            DatabaseCommands::Init(cmd) => cmd.execute(ctx).await,
        }
    }
}
