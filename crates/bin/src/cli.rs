use brontes_database_libmdbx::tables::Tables;
use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
#[command(name = "brontes", author = "Sorella Labs", version = "0.1.0")]
#[command(propagate_version = true)]
pub struct Args {
    #[clap(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Runs brontes
    #[command(name = "run")]
    Run(Run),
    #[command(name = "init")]
    Init(Init),
    #[command(name = "batch-dex")]
    RunBatchWithPricing(RunBatchWithPricing),
}

#[derive(Debug, Parser)]
pub struct Run {
    /// Start Block
    #[arg(long, short)]
    pub start_block: u64,
    /// Optional End Block, if omitted it will continue to run until killed
    #[arg(long, short)]
    pub end_block:   Option<u64>,
    /// Optional Max Tasks, if omitted it will default to 80% of the number of
    /// physical cores on your machine
    pub max_tasks:   Option<u64>,
    /// Optional quote asset, if omitted it will default to USDC
    #[arg(long, short, default_value = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48")]
    pub quote_asset: String,
}

#[derive(Debug, Parser)]
pub struct Init {
    /// Initialize the local Libmdbx DB
    #[arg(long, short, default_value = "true")]
    pub init_libmdbx:         bool,
    /// Libmdbx tables to init:
    ///     TokenDecimals
    ///     AddressToTokens
    ///     AddressToProtocol
    ///     CexPrice
    ///     Metadata
    ///     PoolState
    ///     DexPrice
    #[arg(long, short, requires = "init_libmdbx", value_delimiter = ',')]
    pub tables_to_init:       Option<Vec<Tables>>,
    /// Start Block to download metadata from Sorella's MEV DB
    #[arg(long, short, default_value = "0")]
    pub start_block:          Option<u64>,
    /// End Block to download metadata from Sorella's MEV DB
    #[arg(long, short, default_value = "0")]
    pub end_block:            Option<u64>,
    /// Download Dex Prices from Sorella's MEV DB for the given block range. If
    /// false it will run the dex pricing locally using raw on-chain data
    #[arg(long, short, default_value = "false")]
    pub download_dex_pricing: bool,
}

#[derive(Debug, Parser)]
pub struct RunBatchWithPricing {
    #[arg(long, short)]
    pub start_block:    u64,
    /// Optional End Block, if omitted it will continue to run until killed
    #[arg(long, short)]
    pub end_block:      u64,
    /// Optional Max Tasks, if omitted it will default to 80% of the number of
    /// physical cores on your machine
    pub max_tasks:      Option<u64>,
    /// Optional quote asset, if omitted it will default to USDC
    #[arg(long, short, default_value = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48")]
    pub quote_asset:    String,
    /// how big the batch size should be
    #[arg(long, short, default_value = "500")]
    pub min_batch_size: u64,
}
