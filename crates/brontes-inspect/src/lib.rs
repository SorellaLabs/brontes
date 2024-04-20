//! The `brontes_inspect` crate is designed to efficiently detect and analyze
//! MEV. Emphasizing modularity and ease of use, this crate provides a robust
//! foundation for developing custom inspectors, streamlining the process of MEV
//! strategy identification.
//!
//! By abstracting complex tasks such as decoding, normalization, metadata
//! fetching, and price tracking, `brontes_inspect` allows developers to
//! concentrate on the unique logic of their MEV detection strategies. This
//! design philosophy ensures that users can easily integrate their own
//! inspectors, tailored to specific MEV strategies, without delving
//! into the underlying infrastructure details.
//!
//! ## Inspector
//!
//! `Inspector` is a trait defining a method `process_tree`. This method takes a
//! `BlockTree` and `Metadata` as input and returns a vector of tuples, each
//! containing a `BundleHeader` and a `BundleData`.
//!
//! ```ignore
//! #[async_trait::async_trait]
//! pub trait Inspector: Send + Sync {
//!     type Result: Send + Sync;
//!
//!     async fn process_tree(
//!         &self,
//!         tree: Arc<BlockTree<Actions>>,
//!         metadata: Arc<Metadata>,
//!     ) -> Self::Result;
//! }
//! ```
//!
//! The [`BlockTree`](../brontes-classifier/index.html) represents a block of
//! classified & normalized Ethereum transactions & their traces, and the
//! [`Metadata`](../brontes-database) contains price information & relevant off
//! chain data such as mempool data & centralized exchange price data relevant
//! to that block. The `process_tree` method analyzes the block and identifies
//! instances of the MEV strategy that the inspector is designed to detect.
//!
//! ## Individual Inspectors
//!
//! The `brontes_inspect` crate provides several individual inspectors, each
//! designed to detect a specific type of MEV strategy. These inspectors are
//! defined in their respective modules:
//!
//! - [`atomic_backrun`](atomic_backrun/index.html)
//! - [`cex_dex`](cex_dex/index.html)
//! - [`jit`](jit/index.html)
//! - [`sandwich`](sandwich/index.html)
//! - [`liquidations`](liquidations/index.html)
//! - [`long_tail`](long_tail/index.html)
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
//! ```ignore
//! pub struct Composer<'a, const N: usize> {
//!     inspectors_execution: InspectorFut<'a>,
//!     pre_processing:       BlockPreprocessing,
//! }
//! ```
//!
//! The `Composer` uses  to define a filter that
//! orders results from individual inspectors. This ensures that lower-level
//! actions are composed before higher-level actions, which could affect the
//! composition.
//!
//! Additionally, the `Composer` provides a `Future` implementation for use in
//! asynchronous contexts. When polled, it runs the individual inspectors in
//! parallel and collects their results, processing them to identify complex MEV
//! strategies.
//!
//! In summary, the `brontes_inspect` crate offers tools for detecting and
//! analyzing MEV strategies in Ethereum transactions. Individual inspectors
//! identify specific MEV strategies, while the `Composer` combines these
//! results to identify more complex strategies.

pub mod composer;
pub mod discovery;
pub mod mev_inspectors;
use mev_inspectors::searcher_activity::SearcherActivity;
pub use mev_inspectors::*;

#[cfg(feature = "tests")]
pub mod test_utils;

use std::sync::Arc;

use alloy_primitives::Address;
use atomic_arb::AtomicArbInspector;
use brontes_types::{
    db::{cex::CexExchange, metadata::Metadata, traits::LibmdbxReader},
    mev::{Bundle, BundleData},
    normalized_actions::Actions,
    tree::BlockTree,
};
#[cfg(not(feature = "cex-dex-markout"))]
use cex_dex::CexDexInspector;
// #[cfg(feature = "cex-dex-markout")]
use cex_dex_markout::CexDexMarkoutInspector;
use jit::JitInspector;
use liquidations::LiquidationInspector;
use sandwich::SandwichInspector;

pub trait Inspector: Send + Sync {
    type Result: Send + Sync;

    /// Used for log span so we know which errors come from which inspector
    fn get_id(&self) -> &str;
    fn process_tree(&self, tree: Arc<BlockTree<Actions>>, metadata: Arc<Metadata>) -> Self::Result;
}

#[derive(
    Debug, PartialEq, Clone, Copy, Eq, Hash, strum::Display, strum::EnumString, strum::EnumIter,
)]
pub enum Inspectors {
    AtomicArb,
    #[cfg(not(feature = "cex-dex-markout"))]
    CexDex,
    Jit,
    Liquidations,
    Sandwich,
    SearcherActivity,
    #[cfg(feature = "cex-dex-markout")]
    CexDexMarkout,
}

type DynMevInspector = &'static (dyn Inspector<Result = Vec<Bundle>> + 'static);

impl Inspectors {
    pub fn init_mev_inspector<DB: LibmdbxReader>(
        &self,
        quote_token: Address,
        db: &'static DB,
        cex_exchanges: &[CexExchange],
    ) -> DynMevInspector {
        match &self {
            Self::AtomicArb => {
                static_object(AtomicArbInspector::new(quote_token, db)) as DynMevInspector
            }
            Self::Jit => static_object(JitInspector::new(quote_token, db)) as DynMevInspector,
            #[cfg(not(feature = "cex-dex-markout"))]
            Self::CexDex => static_object(CexDexInspector::new(quote_token, db, cex_exchanges))
                as DynMevInspector,
            Self::Sandwich => {
                static_object(SandwichInspector::new(quote_token, db)) as DynMevInspector
            }
            Self::Liquidations => {
                static_object(LiquidationInspector::new(quote_token, db)) as DynMevInspector
            }
            Self::SearcherActivity => {
                static_object(SearcherActivity::new(quote_token, db)) as DynMevInspector
            }
            #[cfg(feature = "cex-dex-markout")]
            Self::CexDexMarkout => {
                static_object(CexDexMarkoutInspector::new(quote_token, db, cex_exchanges))
                    as DynMevInspector
            }
        }
    }
}

fn static_object<T>(obj: T) -> &'static T {
    &*Box::leak(Box::new(obj))
}
