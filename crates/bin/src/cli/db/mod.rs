use clap::{Parser, Subcommand};

mod r2_uploader;
mod snapshot;
use crate::runner::CliContext;
mod cex_data;
mod clickhouse_download;
mod db_clear;
mod db_insert;
mod db_query;
#[cfg(feature = "local-clickhouse")]
mod discovery;
#[cfg(feature = "local-clickhouse")]
mod ensure_test_traces;
mod export;
mod init;
#[cfg(feature = "local-clickhouse")]
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
    #[command(name = "cex-query")]
    CexData(cex_data::CexDB),
    /// For a given range, will fetch all data from the api and insert it into
    /// libmdbx.
    #[command(name = "init")]
    Init(init::Init),
    /// Export libmbdx data to parquet
    #[command(name = "export")]
    Export(export::Export),
    /// downloads a db snapshot from the remote endpoint
    #[command(name = "download-snapshot")]
    DownloadSnapshot(snapshot::Snapshot),
    #[cfg(feature = "local-clickhouse")]
    /// downloads a the db data from clickhouse
    #[command(name = "download-clickhouse")]
    DownloadClickhouse(clickhouse_download::ClickhouseDownload),
    /// for internal use only. Constantly will upload snapshots
    /// of db every 100k blocks for easy downloads.
    #[command(name = "r2-upload")]
    UploadSnapshot(r2_uploader::R2Uploader),
    #[cfg(feature = "local-clickhouse")]
    /// Traces all blocks needed for testing and inserts them into
    /// clickhouse
    #[command(name = "test-traces-init")]
    TestTracesInit(ensure_test_traces::TestTraceArgs),
    #[cfg(feature = "local-clickhouse")]
    /// Generates traces up to chain tip and inserts them into libmbx
    #[command(name = "trace-at-tip")]
    TraceAtTip(tip_tracer::TipTraceArgs),
    /// from the start block, runs only discovery and inserts into clickhouse.
    /// this ensures we have all classifier data.
    #[cfg(feature = "local-clickhouse")]
    #[command(name = "run-discovery")]
    Discovery(discovery::DiscoveryFill),
}

impl Database {
    pub async fn execute(self, brontes_db_endpoint: String, ctx: CliContext) -> eyre::Result<()> {
        match self.command {
            DatabaseCommands::DbInserts(cmd) => cmd.execute(brontes_db_endpoint).await,
            DatabaseCommands::DbQuery(cmd) => cmd.execute(brontes_db_endpoint).await,
            DatabaseCommands::TraceRange(cmd) => cmd.execute(brontes_db_endpoint, ctx).await,
            DatabaseCommands::Init(cmd) => cmd.execute(brontes_db_endpoint, ctx).await,
            DatabaseCommands::DbClear(cmd) => cmd.execute(brontes_db_endpoint).await,
            DatabaseCommands::UploadSnapshot(cmd) => cmd.execute(brontes_db_endpoint, ctx).await,
            DatabaseCommands::Export(cmd) => cmd.execute(brontes_db_endpoint, ctx).await,
            DatabaseCommands::DownloadSnapshot(cmd) => cmd.execute(brontes_db_endpoint, ctx).await,
            DatabaseCommands::CexData(cmd) => cmd.execute(brontes_db_endpoint, ctx).await,
            #[cfg(feature = "local-clickhouse")]
            DatabaseCommands::DownloadClickhouse(cmd) => {
                cmd.execute(brontes_db_endpoint, ctx).await
            }
            #[cfg(feature = "local-clickhouse")]
            DatabaseCommands::Discovery(cmd) => cmd.execute(brontes_db_endpoint, ctx).await,
            #[cfg(feature = "local-clickhouse")]
            DatabaseCommands::TestTracesInit(cmd) => cmd.execute(brontes_db_endpoint, ctx).await,
            #[cfg(feature = "local-clickhouse")]
            DatabaseCommands::TraceAtTip(cmd) => cmd.execute(brontes_db_endpoint, ctx).await,
        }
    }
}
