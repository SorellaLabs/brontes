//! This module provides testing utilities and fixtures for the
//! `brontes-inspect` crate. It includes support for setting up test scenarios,
//! running inspectors with specific configurations, and validating the outcomes
//! against expected results.
//!
//! ## Modules
//!
//! - `benches`: Contains benchmark tests for performance analysis.
//! - `tests`: Includes the core functionality for setting up and executing
//!   inspector tests.
pub mod benches;
pub use benches::*;

pub mod tests;
pub use tests::*;
