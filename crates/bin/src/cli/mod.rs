use clap::{Parser, Subcommand};

mod db;
mod misc;
mod run;
mod utils;
mod version_data;
pub use utils::*;
pub use version_data::*;

use self::misc::Verbosity;

#[derive(Parser, Debug)]
#[command(author, version = SHORT_VERSION, long_version = LONG_VERSION, about, long_about = None)]
#[command(name = NAME_CLIENT, author = "Sorella Labs", version = "0.1.0")]
#[command(propagate_version = true)]
pub struct Args {
    #[clap(subcommand)]
    pub command:         Commands,
    /// path to the brontes libmdbx db
    #[arg(long = "brontes-db-path", global = true)]
    pub brontes_db_path: Option<String>,
    /// The verbosity level of the logs
    #[clap(flatten)]
    pub verbosity:       Verbosity,
    #[clap(long, default_value = "6923", global = true)]
    pub metrics_port:    u16,
    #[clap(long, default_value = "false", global = true)]
    pub skip_prometheus: bool,
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
