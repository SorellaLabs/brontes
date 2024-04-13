mod searcher_stats;
mod vertical_integration;
use clap::{Parser, Subcommand};
use searcher_stats::GetStats;
use vertical_integration::SearcherBuilder;

use crate::runner::CliContext;

#[derive(Debug, Parser)]
pub struct Analytics {
    #[clap(subcommand)]
    pub command: AnalyticsCommands,
}

#[derive(Debug, Subcommand)]
pub enum AnalyticsCommands {
    /// Identifies vertically integrated searchers & maps them to their builders
    /// in the database
    #[command(name = "detect-vertical-integration", alias = "detect-vi")]
    ViBuilders(SearcherBuilder),
    /// Collects aggregate searcher statistics & prints results
    #[command(name = "searcher-stats", alias = "stats")]
    SearcherStats(GetStats),
}

impl Analytics {
    pub async fn execute(self, brontes_db_endpoint: String, ctx: CliContext) -> eyre::Result<()> {
        match self.command {
            AnalyticsCommands::ViBuilders(cmd) => cmd.execute(brontes_db_endpoint, ctx).await,
            AnalyticsCommands::SearcherStats(cmd) => cmd.execute(brontes_db_endpoint, ctx).await,
        }
    }
}
