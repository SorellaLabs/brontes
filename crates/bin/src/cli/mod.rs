use clap::{Parser, Subcommand};

use crate::cli::ext::{InspectorCliExt, NoopInspectorCliExt};

mod analytics;
mod db;
mod run;
mod utils;

pub use utils::*;
pub mod ext;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
#[command(name = "brontes", author = "Sorella Labs", version = "0.1.0")]
#[command(propagate_version = true)]
pub struct Args<Ext: InspectorCliExt + clap::Args = NoopInspectorCliExt> {
    #[clap(subcommand)]
    pub command: Commands<Ext>,
}

#[derive(Debug, Subcommand)]
pub enum Commands<Ext: InspectorCliExt + clap::Args> {
    /// Run brontes
    #[command(name = "run")]
    Run(run::RunArgs<Ext>),
    /// Brontes database commands
    #[command(name = "db")]
    Database(db::Database),
    /// Brontes Analytics commands
    #[command(name = "analytics")]
    Analytics(analytics::Analytics),
}
