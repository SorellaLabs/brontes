use brontes_types::mev::{compose_sandwich_jit, BundleData, BundleHeader, MevType};
use lazy_static::lazy_static;

/// Defines rules for composing multiple MEV types
/// into a single, complex MEV type.
///
/// This macro creates a static reference (`MEV_COMPOSABILITY_FILTER`) that maps
/// each parent MEV type to a composition function and a list of child MEV
/// types. The composition function is used to combine instances of the child
/// MEV types into a new instance of the parent MEV type.
///
/// # Usage
/// ```ignore
/// mev_composability!(
///     ParentMevType => ChildMevType1, ChildMevType2;
/// );
/// ```
/// In this example, `ParentMevType` is composed of `ChildMevType1` and
/// `ChildMevType2` using a specific composition function.
#[macro_export]
macro_rules! mev_composability {
    ($($parent_mev_type:ident => $($child_mev_type:ident),+;)+) => {
        lazy_static! {
        pub static ref MEV_COMPOSABILITY_FILTER: &'static [(
                MevType,
                ComposeFunction,
                Vec<MevType>)] = {
            &*Box::leak(Box::new([
                $((
                        MevType::$parent_mev_type,
                        get_compose_fn(MevType::$parent_mev_type),
                        [$(MevType::$child_mev_type,)+].to_vec()),
                   )+
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

// Defines precedence rules among different MEV types
/// for the purpose of deduplication.
///
/// This macro creates a static reference (`MEV_DEDUPLICATION_FILTER`) that maps
/// each dominant MEV type to a list of subordinate MEV types. These rules are
/// used to determine which MEV types should be considered for deduplication
/// when multiple types are present for overlapping transactions.
///
/// # Usage
/// ```ignore
/// define_mev_precedence!(
///     DominantMevType => SubordinateMevType1, SubordinateMevType2;
/// );
/// ```
/// In this example, `DominantMevType` takes precedence over
/// `SubordinateMevType1` and `SubordinateMevType2` for deduplication purposes.
#[macro_export]
macro_rules! define_mev_precedence {
    ($($dominant_mev_type:ident => $($subordinate_mev_type:ident),+;)+) => {
        lazy_static! {
        pub static ref MEV_DEDUPLICATION_FILTER: &'static [(
                MevType,
                Vec<MevType>)] = {
            &*Box::leak(Box::new([
                $((
                        MevType::$dominant_mev_type,
                        [$(MevType::$subordinate_mev_type,)+].to_vec()),
                   )+
            ]))
        };
    }
    };
}
