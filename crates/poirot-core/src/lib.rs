use std::sync::atomic::AtomicUsize;

use colored::{Colorize, ColoredString, Color};
use structured_trace::StructuredTrace;
use std::fmt::Debug;

pub mod decode;
pub mod errors;
pub mod normalize;
pub mod stats;
pub mod structured_trace;


pub static SUCCESSFUL_TRACE_PARSE: &'static str = "Successfully Parsed Trace";
pub static SUCCESSFUL_TX_PARSE: &'static str = "Successfully Parsed Transaction";
pub static TRANSACTION_COUNTER: AtomicUsize = AtomicUsize::new(0);
pub static TRACE_COUNTER: AtomicUsize = AtomicUsize::new(0);
pub static SUCCESSFUL_PARSE_COUNTER: AtomicUsize = AtomicUsize::new(0);
