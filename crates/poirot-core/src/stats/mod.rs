use colored::{ColoredString, Color, Colorize};
use std::fmt::Debug;

pub mod macros;
pub mod stats;
pub mod display;


/// formats a stat with a color based on its value + kind
pub fn format_color(stat: &str, val: impl Debug, color: Color) -> ColoredString {
    format!("{}: {:?}", stat, val).color(color)
}