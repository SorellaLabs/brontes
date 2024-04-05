//TODO: (Ludwig) Finish this once all other crates have been documented.

//! This is the main binary crate for the Brontes project. It uses several other
//! crates in the workspace, which are documented separately:
//!
//! - [brontes-core](../brontes_core/index.html): Tracing for the Brontes
//!   project.
//! - [brontes-inspect](../brontes_inspect/index.html): Mev Inspectors for MEV
//!   detection.
//! - [brontes-types](../brontes_types/index.html): Defines the main types used
//!   across Brontes.
//! - [brontes-classifier](../brontes_classifier/index.html): Contains the
//!   classifier logic pertaining to transaction trace classification &
//!   normalization.
//! - [brontes-metrics](../brontes_metrics/index.html): Handles metrics
//!   collection and reporting.
//! - [brontes-pricing](../brontes_pricing/index.html): Handles DEX pricing
//! - [brontes-database](../brontes_database/index.html): Handles database
//!   related functionalities.
//! - [reth-tracing-ext](../reth_tracing_ext/index.html): Provides extended
//!   tracing capabilities to match transaction traces to their corresponding
//!   logs.
//!
//! Please refer to the individual crate documentation for more details.

pub mod cli;
pub mod executors;
pub mod misc;
pub use executors::*;
pub use misc::banner;

pub mod runner;
//TUI related
pub mod tui;
