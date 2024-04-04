

use brontes_analytics::BrontesAnalytics;

use brontes_types::mev::bundle::MevType;
use clap::Parser;


use crate::{
    cli::{
        determine_max_tasks, init_brontes_db, init_tracer,
    },
    runner::CliContext,
};
// Convert to polars notebook code:
// bundle count by mev_type by builder
// bundle value by mev_type by builder

#[derive(Debug, Parser)]
pub struct SearcherBuilder {
    /// Start Block
    #[arg(long, short)]
    pub start_block: u64,
    /// Optional End Block, if omitted it will continue to run until killed
    #[arg(long, short)]
    pub end_block:   u64,
    /// Optional Max Tasks, if omitted it will default to 80% of the number of
    /// physical cores on your machine
    #[arg(long, short)]
    pub max_tasks:   Option<u64>,
    /// Optional MevType to filter by (e.g. only CexDex bundles will be
    /// considered when identifying searcher to builder relationships)
    #[arg(long, short, value_delimiter = ',')]
    pub mev_type:    Option<Vec<MevType>>,
}

impl SearcherBuilder {
    pub async fn execute(self, ctx: CliContext) -> eyre::Result<()> {
        let libmdbx = init_brontes_db()?;

        let task_executor = ctx.task_executor;

        let max_tasks = determine_max_tasks(self.max_tasks);
        let tracer = init_tracer(task_executor.clone(), max_tasks)?;

        let brontes_analytics = BrontesAnalytics::new(libmdbx, tracer.clone(), None);

        brontes_analytics
            .get_vertically_integrated_searchers(self.start_block, self.end_block, self.mev_type)
            .await?;

        Ok(())
    }
}
