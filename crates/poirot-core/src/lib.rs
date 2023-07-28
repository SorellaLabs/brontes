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


/// formats a stat with a color based on its value + kind
pub fn format_color(stat: &str, val: impl Debug, color: Color) -> ColoredString {
    format!("{}: {:?}", stat, val).color(color)
}


pub fn str_trace_action(trace: &StructuredTrace) -> String {
    match trace {
        StructuredTrace::CALL(_) => "Call".to_string(),
        StructuredTrace::CREATE(_) => "Create".to_string(),
        StructuredTrace::SELFDESTRUCT(_) => "Self Destruct".to_string(),
        StructuredTrace::REWARD(_) => "Reward".to_string(),
    }
}