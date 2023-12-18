use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct Opts {
    #[clap(subcommand)]
    pub sub: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Runs brontes
    Brontes(Cli),
}

#[derive(Debug, Parser)]
pub struct Cli {
    /// Start Block
    #[arg(long, short)]
    pub start_block:  u64,
    /// Optional End Block, if omitted it will continue to run until killed
    #[arg(long, short)]
    pub end_block:    Option<u64>,
    /// Max Block Queue size
    #[arg(long, short, default_value = "10")]
    pub max_tasks:    u64,
    /// Flush Tardis data loaded into clickhouse upon termination
    #[arg(long, short, default_value = "false")]
    pub flush_tardis: bool,
    /// initializes libmdbx tables
    #[arg(long, short, default_value = "false")]
    pub init_libmdbx: bool,
    /// Will run in test mode, benchmarking the perfomance of the inspectors
    /// against our latest best run
    #[arg(long, short, default_value = "false")]
    pub test:         bool,
}
