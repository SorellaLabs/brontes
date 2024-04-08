use brontes_types::mev::{compose_sandwich_jit, Bundle, MevType};
use lazy_static::lazy_static;
/// Defines rules for composing multiple child MEV types into a single, complex
/// parent MEV type.
///
/// This macro creates a static reference (`MEV_COMPOSABILITY_FILTER`) that maps
/// a list of child MEV types to each parent MEV type along with a composition
/// function. The composition function is used to combine instances of the child
/// MEV types into a new instance of the parent MEV type.
///
/// # Usage
/// ```ignore
/// mev_composability!(
///     ChildMevType1, ChildMevType2 => ParentMevType;
/// );
/// ```
/// In this example, `ParentMevType` is composed of `ChildMevType1` and
/// `ChildMevType2` using a specific composition function.
#[macro_export]
macro_rules! mev_composability {
    ($($($child_mev_type:ident),+ => $parent_mev_type:ident;)+) => {
        lazy_static! {
            pub static ref MEV_COMPOSABILITY_FILTER:
                &'static [(MevType, ComposeFunction, Vec<MevType>)] = {
                &*Box::leak(Box::new([
                    $((
                        MevType::$parent_mev_type,
                        get_compose_fn(MevType::$parent_mev_type),
                        vec![$(MevType::$child_mev_type),+],
                    ),)+
                ]))
            };
        }
    };
}

pub type ComposeFunction = Box<dyn Fn(Vec<Bundle>) -> Bundle + Send + Sync>;

pub fn get_compose_fn(mev_type: MevType) -> ComposeFunction {
    match mev_type {
        MevType::JitSandwich => Box::new(compose_sandwich_jit),
        _ => unreachable!("This mev type does not have a compose function"),
    }
}

mev_composability!(
    Sandwich, Jit => JitSandwich;
);

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
    Unknown, SearcherTx => CexDex;
    Unknown, SearcherTx, CexDex => AtomicArb;
    Unknown, SearcherTx, AtomicArb, CexDex => Sandwich;
    Unknown, SearcherTx, AtomicArb, CexDex => Jit;
    Unknown, SearcherTx, AtomicArb, CexDex, Sandwich => JitSandwich;
    Unknown, SearcherTx, AtomicArb, CexDex => Liquidation;
);
