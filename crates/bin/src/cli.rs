use clap::{Parser, Subcommand};
use colored::Colorize;
use indoc::indoc;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct Opts {
    #[clap(subcommand)]
    pub sub: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Runs mev-poirot
    Poirot(Cli),
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
    #[arg(long, short, default_value = "1000")]
    pub max_tasks:    usize,
    /// Flush Tardis data loaded into clickhouse upon termination
    #[arg(long, short, default_value = "false")]
    pub flush_tardis: bool,
    /// Will run in test mode, benchmarking the perfomance of the inspectors
    /// against our latest best run
    #[arg(long, short, default_value = "false")]
    pub test:         bool,
}

pub fn print_banner() {
    let banner = indoc! {
    r#"
    /***
     *    •f▌f▄f·.f▄▄▄f.f▌f▐·f▄▄▄·ffffff▪ff▄▄▄ffffffff▄▄▄▄▄
     *    ·██f▐███▪▀▄.▀·▪█·█▌▐█f▄█▪fffff██f▀▄f█·▪fffff•██ff
     *    ▐█f▌▐▌▐█·▐▀▀▪▄▐█▐█•f██▀·f▄█▀▄f▐█·▐▀▀▄ff▄█▀▄ff▐█.▪
     *    ██f██▌▐█▌▐█▄▄▌f███f▐█▪·•▐█▌.▐▌▐█▌▐█•█▌▐█▌.▐▌f▐█▌·
     *    ▀▀ff█▪▀▀▀f▀▀▀f.f▀ff.▀ffff▀█▄▀▪▀▀▀.▀ff▀f▀█▄▀▪f▀▀▀f
     */
    "#};
    println!("{}", format!("{}", banner.red().bold()));
}
