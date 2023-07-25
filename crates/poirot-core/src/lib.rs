use colored::{Colorize, ColoredString};

pub mod decode;
pub mod errors;
pub mod normalize;
pub mod stats;
pub mod structured_trace;

/// formats a stat with a color based on its value
pub fn format_color(stat: &str, val: usize) -> ColoredString {
    if val == 0 {
        format!("{}", stat).bright_green()
    } else {
        format!("{}", stat).bright_red()
    }
}