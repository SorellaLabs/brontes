use clap::{Parser, Subcommand};

mod analytics;
mod db;
mod run;
mod utils;

pub use utils::*;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
#[command(name = "brontes", author = "Sorella Labs", version = "0.1.0")]
#[command(propagate_version = true)]
pub struct Args {
    #[clap(subcommand)]
    pub command:         Commands,
    /// path to the brontes libmdbx db
    #[arg(long = "brontes-db-path", global = true)]
    pub brontes_db_path: Option<String>,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Run brontes
    #[command(name = "run")]
    Run(run::RunArgs),
    /// Brontes database commands
    #[command(name = "db")]
    Database(db::Database),
    /// Brontes Analytics commands
    #[command(name = "analytics")]
    Analytics(analytics::Analytics),
}
