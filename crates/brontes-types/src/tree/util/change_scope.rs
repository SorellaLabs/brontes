use std::marker::PhantomData;

use crate::{normalized_actions::NormalizedAction, ScopeIter, TreeIter};

/// wrapper around any scope iter to change what it expresses without lossing
/// data
// pub struct ChangeScope<V: NormalizedAction, I: ScopeIter<V>> {
//     iter: I,
//     _p:   PhantomData<V>,
// }

/// allows changing scope of base iters
pub trait ChangeScope<V: NormalizedAction, Keys, Out>: ScopeIter<V>
where
    Out: ScopeIter<V>,
{
    fn change_scope(self, k: Keys) -> Out;
}

macro_rules! change_scope {
    ($i:tt, ) => {
        paste::paste!();
    };
}
