use brontes_types::mev::MevType;
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
                &'static [(MevType, Vec<MevType>)] = {
                &*Box::leak(Box::new([
                    $((
                        MevType::$dominant_mev_type,
                        vec![$(MevType::$subordinate_mev_type),+],
                    ),)+
                ]))
            };
        }
    };
}

define_mev_precedence!(
    Unknown, SearcherTx, Jit=> CexDex;
    Unknown, SearcherTx, CexDex => AtomicArb;
    Unknown, SearcherTx, AtomicArb=> Jit;
    Unknown, SearcherTx, AtomicArb, CexDex => Liquidation;
    Unknown, SearcherTx, AtomicArb, CexDex => Sandwich;
    Unknown, SearcherTx, AtomicArb, CexDex, Jit, Sandwich => JitSandwich;
);
