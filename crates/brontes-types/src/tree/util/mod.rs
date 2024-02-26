pub mod collect;
pub mod filter;

pub use collect::*;
pub use filter::*;

use crate::normalized_actions::NormalizedAction;

pub trait TreeOperation<V: NormalizedAction> {}
