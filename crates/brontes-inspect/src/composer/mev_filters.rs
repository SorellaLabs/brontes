use std::sync::Arc;

use brontes_types::{
    db::traits::LibmdbxReader,
    mev::{AtomicArbType, Bundle, BundleData, MevType},
    normalized_actions::Action,
    BlockTree,
};
use lazy_static::lazy_static;

/// Defines precedence rules among different MEV types for the purpose of
/// deduplication.
///
/// This macro creates a static reference (`MEV_DEDUPLICATION_FILTER`) that maps
/// a list of subordinate MEV types to each dominant MEV type. These rules are
/// used to determine which MEV types should be considered for deduplication
/// when multiple types are present for overlapping transactions.
///
/// # Usage
/// ```ignore
/// define_mev_precedence!(
///     SubordinateMevType1, SubordinateMevType2 => DominantMevType;
/// );
/// ```
/// In this example, `DominantMevType` takes precedence over
/// `SubordinateMevType1` and `SubordinateMevType2` for deduplication purposes.
///
/// Example of defining multiple precedence rules:
/// ```ignore
/// define_mev_precedence!(
///     Backrun => Sandwich;
///     Backrun => Jit;
///     Backrun => JitSandwich;
/// );
/// ```
/// In these examples, `Backrun` is considered subordinate to `Sandwich`, `Jit`,
/// `JitSandwich`.  
#[macro_export]
macro_rules! define_mev_precedence {
    ($($($subordinate_mev_type:ident),+ => $dominant_mev_type:ident;)+) => {
        lazy_static! {
            pub static ref MEV_DEDUPLICATION_FILTER:
                &'static [(MevType, FilterFn, Vec<MevType>)] = {
                &*Box::leak(Box::new([
                    $((
                        MevType::$dominant_mev_type,
                        get_filter_fn(MevType::$dominant_mev_type),
                        vec![$(MevType::$subordinate_mev_type),+],
                    ),)+
                ]))
            };
        }
    };
}

pub type FilterFn = Option<
    Box<
        dyn Fn(Arc<BlockTree<Action>>, Arc<Box<dyn LibmdbxReader>>, [&Bundle; 2]) -> bool
            + Send
            + Sync,
    >,
>;

pub fn get_filter_fn(mev_type: MevType) -> FilterFn {
    match mev_type {
        MevType::AtomicArb => Some(Box::new(atomic_dedup_fn)),
        _ => None,
    }
}

/// returns true if should dedup.
pub fn atomic_dedup_fn(
    _tree: Arc<BlockTree<Action>>,
    _db: Arc<Box<dyn LibmdbxReader>>,
    bundles: [&Bundle; 2],
) -> bool {
    let [atomic, other] = bundles;

    if matches!(other.data, BundleData::CexDex(_)) {
        let atomic_data = match &atomic.data {
            BundleData::AtomicArb(data) => data,
            _ => {
                return false;
            }
        };

        if atomic_data.arb_type == AtomicArbType::Triangle {
            return false;
        }
        // if the cex dex has a higher value. then use that.
        if other.header.profit_usd >= atomic.header.profit_usd {
            return false;
        }
        // if the cex dex isn't a known fund ignore
        if !other.header.fund.is_none() {
            return false;
        }
    }

    true
}

define_mev_precedence!(
    // will filter out unless function says otherwise
    CexDexTrades => AtomicArb;

    // filter out all atomic arbs that we kept as cex dex
    AtomicArb => CexDexTrades;
    Unknown, SearcherTx => CexDexQuotes;
    Unknown, SearcherTx => CexDexTrades;
    Unknown, SearcherTx => AtomicArb;
    Unknown, SearcherTx, AtomicArb => Jit;
    Unknown, SearcherTx, AtomicArb, CexDexQuotes,CexDexTrades  => Liquidation;
    Unknown, SearcherTx, AtomicArb, CexDexQuotes,CexDexTrades  => Sandwich;
    Unknown, SearcherTx, AtomicArb, Jit, CexDexQuotes, CexDexTrades=> JitCexDex;
    Unknown, SearcherTx, AtomicArb, CexDexQuotes, CexDexTrades, Jit, Sandwich => JitSandwich;
);
