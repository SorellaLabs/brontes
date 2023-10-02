use clap::{Args, Parser, Subcommand};
use std::Debug;
use indoc::indoc;
use colored::Colorize;

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Runs mev-poirot
    #[command(name = "poirot")]
    poirot(Cli),
}

#[derive(Debug, Parser)]
pub struct Cli {
    /// Start Block
    #[arg(long, short)]
    pub start_block: u64,
    /// Optional End Block, if omitted it will continue to run until killed
    #[arg(long, short)]
    pub end_block: Option<u64>,
    /// Flush Tardis data loaded into clickhouse upon termination
    #[arg(long, short, default_value = "false")]
    pub flush_tardis: bool,
    /// Will run in test mode, benchmarking the perfomance of the inspectors against our latest best run
    #[arg(long, short, default_value = "false")]
    pub test: bool,
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
    println!(format!("{}", banner.red().bold()));
}
