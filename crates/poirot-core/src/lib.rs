use colored::{Colorize, ColoredString};
use structured_trace::StructuredTrace;

pub mod decode;
pub mod errors;
pub mod normalize;
pub mod stats;
pub mod structured_trace;

/// formats a stat with a color based on its value + kind
pub fn format_color(stat: &str, val: usize, error: bool) -> ColoredString {
    if val != 0 {
        if error {
            format!("{}: {}", stat, val).bright_red().bold()
        } else {
            format!("{}: {}", stat, val).bright_green().bold()
        }
    } else {
        if error {
            format!("{}: {}", stat, val).bright_green().bold()
        } else {
            format!("{}: {}", stat, val).bright_yellow().bold()
        }
    }
}


pub fn str_trace_action(trace: &StructuredTrace) -> String {
    match trace {
        StructuredTrace::CALL(_) => "Call".to_string(),
        StructuredTrace::CREATE(_) => "Create".to_string(),
        StructuredTrace::SELFDESTRUCT(_) => "Self Destruct".to_string(),
        StructuredTrace::REWARD(_) => "Reward".to_string(),
    }
}