//! The `brontes_inspect` crate is designed to efficiently detect and analyze
//! a block. Emphasizing modularity and ease of use, this crate provides a
//! robust foundation for developing custom inspectors, streamlining the process
//! of complex transaction & block analysis.
//!
//! ## Inspector
//!
//! `Inspector` is a trait defining a method `inspect_block`. This method takes
//! a `BlockTree` and `Metadata` as input and returns a vector of tuples, each
//! containing a `BundleHeader` and a `BundleData`.
//!
//! ```ignore
//! #[async_trait::async_trait]
//! pub trait Inspector: Send + Sync {
//!     type Result: Send + Sync;
//!
//!     async fn inspect_block(
//!         &self,
//!         tree: Arc<BlockTree<Action>>,
//!         metadata: Arc<Metadata>,
//!     ) -> Self::Result;
//! }
//! ```
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
//! implementation of the `inspect_block` method.
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

pub mod composer;
pub mod discovery;
pub mod mev_inspectors;
use brontes_metrics::inspectors::{OutlierMetrics, ProfitMetrics};
use mev_inspectors::searcher_activity::SearcherActivity;
pub use mev_inspectors::*;

#[cfg(feature = "tests")]
pub mod test_utils;

use alloy_primitives::Address;
use atomic_arb::AtomicArbInspector;
use brontes_types::{
    db::{
        cex::{trades::CexDexTradeConfig, CexExchange},
        metadata::Metadata,
        traits::LibmdbxReader,
    },
    mev::{Bundle, BundleData},
    normalized_actions::Action,
    tree::BlockTree,
    MultiBlockData,
};
use cex_dex::{markout::CexDexMarkoutInspector, quotes::CexDexQuotesInspector};
use jit::JitCexDex;
use liquidations::LiquidationInspector;
use sandwich::SandwichInspector;

use crate::jit::jit_liquidity::JitInspector;

pub trait Inspector: Send + Sync {
    type Result: Send + Sync;

    /// default is 1
    fn block_window(&self) -> usize {
        1
    }
    /// Used for log span so we know which errors come from which inspector
    fn get_id(&self) -> &str;
    fn inspect_block(&self, data: MultiBlockData) -> Self::Result;
    fn get_quote_token(&self) -> Address;
}

#[derive(
    Debug, PartialEq, Clone, Copy, Eq, Hash, strum::Display, strum::EnumString, strum::EnumIter,
)]
pub enum Inspectors {
    AtomicArb,
    CexDex,
    Jit,
    Liquidations,
    Sandwich,
    SearcherActivity,
    CexDexMarkout,
    JitCexDex,
}

type DynMevInspector = &'static (dyn Inspector<Result = Vec<Bundle>> + 'static);

impl Inspectors {
    pub fn init_mev_inspector<DB: LibmdbxReader>(
        &self,
        quote_token: Address,
        db: &'static DB,
        cex_exchanges: &[CexExchange],
        trade_config: CexDexTradeConfig,
        metrics: Option<OutlierMetrics>,
        profit_metrics: Option<ProfitMetrics>,
    ) -> DynMevInspector {
        match &self {
            Self::AtomicArb => {
                static_object(AtomicArbInspector::new(quote_token, db, metrics, profit_metrics))
                    as DynMevInspector
            }
            Self::Jit => static_object(JitInspector::new(quote_token, db, metrics, profit_metrics))
                as DynMevInspector,

            Self::CexDex => static_object(CexDexQuotesInspector::new(
                quote_token,
                db,
                cex_exchanges,
                trade_config.quote_offset_from_block_us,
                metrics,
                profit_metrics,
            )) as DynMevInspector,
            Self::Sandwich => {
                static_object(SandwichInspector::new(quote_token, db, metrics, profit_metrics))
                    as DynMevInspector
            }
            Self::Liquidations => {
                static_object(LiquidationInspector::new(quote_token, db, metrics, profit_metrics))
                    as DynMevInspector
            }
            Self::SearcherActivity => {
                static_object(SearcherActivity::new(quote_token, db, metrics, profit_metrics))
                    as DynMevInspector
            }
            Self::CexDexMarkout => static_object(CexDexMarkoutInspector::new(
                quote_token,
                db,
                cex_exchanges,
                trade_config,
                metrics,
                profit_metrics,
            )) as DynMevInspector,
            Self::JitCexDex => static_object(JitCexDex {
                cex_dex: CexDexMarkoutInspector::new(
                    quote_token,
                    db,
                    cex_exchanges,
                    trade_config,
                    metrics.clone(),
                    profit_metrics.clone(),
                ),
                jit:     JitInspector::new(quote_token, db, metrics, profit_metrics),
            }) as DynMevInspector,
        }
    }
}

fn static_object<T>(obj: T) -> &'static T {
    &*Box::leak(Box::new(obj))
}
