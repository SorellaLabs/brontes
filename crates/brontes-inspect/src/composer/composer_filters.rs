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

pub type ComposeFunction = Box<dyn Fn(Vec<Bundle>) -> Option<Bundle> + Send + Sync>;

pub fn get_compose_fn(mev_type: MevType) -> ComposeFunction {
    match mev_type {
        MevType::JitSandwich => Box::new(compose_sandwich_jit),
        _ => unreachable!("This mev type does not have a compose function"),
    }
}

mev_composability!(
    Sandwich, Jit => JitSandwich;
);
