use colored::{Color, ColoredString, Colorize};
use std::fmt::Debug;

pub mod display;
pub mod macros;
#[allow(clippy::module_inception)]
pub mod stats;

/// formats a stat with a color based on its value + kind
pub fn format_color(stat: &str, val: impl Debug, color: Color) -> ColoredString {
    format!("{}: {:?}", stat, val).color(color)
}
