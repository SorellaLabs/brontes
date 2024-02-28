use crate::{normalized_actions::NormalizedAction, ScopeIter};

impl<V: NormalizedAction, T: ScopeIter<V> + Sized> ScopeCollect<V> for T {}

pub trait ScopeCollect<V: NormalizedAction>: ScopeIter<V> {
    fn collect<I, Out: Default + Extend<I>>(self) -> Out
    where
        Self: Sized,
    {
        todo!()
    }
}
