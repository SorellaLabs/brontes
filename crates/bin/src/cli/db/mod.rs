use clap::{Parser, Subcommand};
mod r2_uploader;
mod snapshot;
use crate::runner::CliContext;
mod cex_data;
#[cfg(feature = "local-clickhouse")]
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
mod table_stats;
#[cfg(feature = "local-clickhouse")]
mod tip_tracer;
mod trace_range;
pub mod utils;

#[derive(Debug, Parser)]
pub struct Database {
    #[clap(subcommand)]
    pub command: DatabaseCommands,
}

#[derive(Debug, Subcommand)]
pub enum DatabaseCommands {
    /// Insert into the brontes libmdbx db
    #[command(name = "insert")]
    DbInserts(db_insert::Insert),
    /// Query data from any libmdbx table and pretty print it in stdout
    #[command(name = "query")]
    DbQuery(db_query::DatabaseQuery),
    /// Clear a libmdbx table
    #[command(name = "clear")]
    DbClear(db_clear::Clear),
    /// Generates traces and store them in libmdbx (also clickhouse if
    /// --feature local-clickhouse)
    #[command(name = "generate-traces")]
    TraceRange(trace_range::TraceArgs),
    /// Fetches Cex data from the Sorella DB
    #[command(name = "cex-query")]
    CexData(cex_data::CexDB),
    /// Fetch data from the api and insert it into
    /// libmdbx.
    #[command(name = "init")]
    Init(init::Init),
    /// Libmbdx Table Stats
    #[command(name = "table-stats")]
    TableStats(table_stats::Stats),
    /// Export libmbdx data to parquet
    #[command(name = "export")]
    Export(export::Export),
    /// Downloads a database snapshot. Without specified blocks, it fetches
    /// the full range. With start/end blocks, it downloads that range and
    /// merges it into the current database.
    #[command(name = "download-snapshot")]
    DownloadSnapshot(snapshot::Snapshot),
    #[cfg(feature = "local-clickhouse")]
    /// Downloads the db data from clickhouse
    #[command(name = "download-clickhouse")]
    DownloadClickhouse(clickhouse_download::ClickhouseDownload),
    /// For internal use only. Uploads snapshots
    /// of db every 100k blocks to r2
    #[command(name = "r2-upload")]
    UploadSnapshot(r2_uploader::R2Uploader),
    #[cfg(feature = "local-clickhouse")]
    /// Traces all blocks required to run the tests and inserts them into
    /// clickhouse
    #[command(name = "test-traces-init")]
    TestTracesInit(ensure_test_traces::TestTraceArgs),
    #[cfg(feature = "local-clickhouse")]
    /// Generates traces up to chain tip and inserts them into libmbx
    #[command(name = "trace-at-tip")]
    TraceAtTip(tip_tracer::TipTraceArgs),
    /// Only runs discovery and inserts discovered protocols into clickhouse
    #[cfg(feature = "local-clickhouse")]
    #[command(name = "run-discovery")]
    Discovery(discovery::DiscoveryFill),
}

impl Database {
    pub async fn execute(self, brontes_db_path: String, ctx: CliContext) -> eyre::Result<()> {
        match self.command {
            DatabaseCommands::DbInserts(cmd) => cmd.execute(brontes_db_path).await,
            DatabaseCommands::DbQuery(cmd) => cmd.execute(brontes_db_path).await,
            DatabaseCommands::TraceRange(cmd) => cmd.execute(brontes_db_path, ctx).await,
            DatabaseCommands::Init(cmd) => cmd.execute(brontes_db_path, ctx).await,
            DatabaseCommands::DbClear(cmd) => cmd.execute(brontes_db_path).await,
            DatabaseCommands::UploadSnapshot(cmd) => cmd.execute(brontes_db_path, ctx).await,
            DatabaseCommands::Export(cmd) => cmd.execute(brontes_db_path, ctx).await,
            DatabaseCommands::TableStats(cmd) => cmd.execute(brontes_db_path),
            DatabaseCommands::DownloadSnapshot(cmd) => cmd.execute(brontes_db_path, ctx).await,
            DatabaseCommands::CexData(cmd) => cmd.execute(brontes_db_path, ctx).await,
            #[cfg(feature = "local-clickhouse")]
            DatabaseCommands::DownloadClickhouse(cmd) => cmd.execute(brontes_db_path, ctx).await,
            #[cfg(feature = "local-clickhouse")]
            DatabaseCommands::Discovery(cmd) => cmd.execute(brontes_db_path, ctx).await,
            #[cfg(feature = "local-clickhouse")]
            DatabaseCommands::TestTracesInit(cmd) => cmd.execute(brontes_db_path, ctx).await,
            #[cfg(feature = "local-clickhouse")]
            DatabaseCommands::TraceAtTip(cmd) => cmd.execute(brontes_db_path, ctx).await,
        }
    }
}
