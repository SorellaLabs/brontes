//! # Brontes Inspect Crate
//!
//! The `brontes_inspect` crate is a part of the Brontes system, designed to
//! detect and analyze Miner Extractable Value (MEV) strategies in Ethereum
//! transactions. This crate provides a set of inspectors, each responsible for
//! identifying specific types of MEV strategies.
//!
//! ## Inspector
//!
//! `Inspector` is a trait defining a method `process_tree`. This method takes a
//! `BlockTree` and `Metadata` as input and returns a vector of tuples, each
//! containing a `ClassifiedMev` and a `SpecificMev`.
//!
//! ```
//! #[async_trait::async_trait]
//! pub trait Inspector: Send + Sync {
//!     async fn process_tree(
//!         &self,
//!         tree: Arc<BlockTree<Actions>>,
//!         metadata: Arc<Metadata>,
//!     ) -> Vec<(ClassifiedMev, Box<dyn SpecificMev>)>;
//! }
//! ```
//!
//! The `BlockTree` represents a block of Ethereum transactions, and the
//! `Metadata` contains additional information about the block. The
//! `process_tree` method analyzes the block and identifies instances of the MEV
//! strategy that the inspector is designed to detect.
//!
//! ## Individual Inspectors
//!
//! The `brontes_inspect` crate provides several individual inspectors, each
//! designed to detect a specific type of MEV strategy. These inspectors are
//! defined in their respective modules:
//!
//! - `atomic_backrun`
//! - `cex_dex`
//! - `jit`
//! - `sandwich`
//!
//! Each inspector implements the `Inspector` trait and provides its own
//! implementation of the `process_tree` method.
//!
//! ## Composer
//!
//! The `Composer` is a special type of inspector that combines the results of
//! individual inspectors to identify more complex MEV strategies. It takes an
//! array of individual inspectors and a `BlockTree` and `Metadata` as input,
//! running each inspector on the block and collecting their results.
//!
//! ```
//! pub struct Composer<'a, const N: usize> {
//!     inspectors_execution: InspectorFut<'a>,
//!     pre_processing:       BlockPreprocessing,
//! }
//! ```
//!
//! The `Composer` uses a macro `mev_composability` to define a filter that
//! orders results from individual inspectors. This ensures that lower-level
//! actions are composed before higher-level actions, which could affect the
//! composition.
//!
//! Additionally, the `Composer` provides a `Future` implementation for use in
//! asynchronous contexts. When polled, it runs the individual inspectors on the
//! block and collects their results, processing them to identify complex MEV
//! strategies.
//!
//! In summary, the `brontes_inspect` crate offers tools for detecting and
//! analyzing MEV strategies in Ethereum transactions. Individual inspectors
//! identify specific MEV strategies, while the `Composer` combines these
//! results to identify more complex strategies.

pub mod atomic_backrun;
pub mod cex_dex;
pub mod composer;
pub mod jit;
#[allow(dead_code, unused_imports)]
pub mod liquidations;
pub mod sandwich;
pub mod shared_utils;

use std::sync::Arc;

use brontes_database::Metadata;
use brontes_types::{
    classified_mev::{ClassifiedMev, SpecificMev},
    normalized_actions::Actions,
    tree::BlockTree,
};

#[async_trait::async_trait]
pub trait Inspector: Send + Sync {
    async fn process_tree(
        &self,
        tree: Arc<BlockTree<Actions>>,
        metadata: Arc<Metadata>,
    ) -> Vec<(ClassifiedMev, Box<dyn SpecificMev>)>;
}
