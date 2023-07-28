use colored::{Color, ColoredString, Colorize};
use lazy_static::*;
use revm_primitives::B256;
use std::{fmt::Debug, collections::HashMap, sync::Mutex};
use crate::stats::stats::*;

pub mod display;
pub mod macros;
#[allow(clippy::module_inception)]
pub mod stats;


// block and transaction stats
lazy_static! {
    pub static ref BLOCK_STATS: Mutex<HashMap<u64, BlockStats>> = Mutex::new(HashMap::new());
    pub static ref TX_STATS: Mutex<HashMap<B256, TransactionStats>> =
        Mutex::new(HashMap::new());
}


/// formats a stat with a color based on its value + kind
pub fn format_color(stat: &str, val: impl Debug, color: Color) -> ColoredString {
    format!("{}: {:?}", stat, val).color(color)
}
