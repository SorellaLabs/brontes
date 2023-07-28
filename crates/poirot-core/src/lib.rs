use colored::Colorize;
use revm_primitives::B256;
use std::{fmt::Debug, collections::HashMap, sync::Mutex};
use lazy_static::*;

use crate::stats::stats::*;

pub mod decode;
pub mod errors;
pub mod normalize;
pub mod stats;
pub mod structured_trace;


// block and transaction stats
lazy_static! {
    pub static ref BLOCK_STATS: Mutex<HashMap<u64, BlockStats>> = Mutex::new(HashMap::new());
    pub static ref TX_STATS: Mutex<HashMap<B256, TransactionStats>> =
        Mutex::new(HashMap::new());
}