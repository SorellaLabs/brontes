use clap::{Parser, Subcommand};

mod db;
mod misc;
mod run;
mod utils;

pub use utils::*;

use self::misc::Verbosity;

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
    /// verbosity fo the logs
    #[clap(flatten)]
    pub verbosity:       Verbosity,
    #[clap(long, default_value = "6923")]
    pub metrics_port:    u16,
}

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Run brontes
    #[command(name = "run")]
    Run(run::RunArgs),
    /// Brontes database commands
    #[command(name = "db")]
    Database(db::Database),
}
