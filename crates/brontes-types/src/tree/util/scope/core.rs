use crate::{
    normalized_actions::NormalizedAction, Map, ScopeIter, TreeIter, TreeIterator,
    TreeIteratorScope, TreeMap, TreeMapAll,
};

impl<V: NormalizedAction, U: Iterator, T: TreeIter<V> + ScopeIter<U>> TreeScoped<V, U> for T where
    T: Sized
{
}

pub trait TreeScoped<V: NormalizedAction, U: Iterator>: TreeIter<V> + ScopeIter<U> {
    fn tree_map_all<Keys, Out, F, O: Iterator>(self, f: F) -> Out
    where
        Self: Sized + TreeMapAll<V, Out, Keys, F>,
        Out: ScopeIter<O> + TreeIter<V>,
    {
        TreeMapAll::tree_map_all(self, f)
    }

    fn tree_map<Keys, Out, F, O: Iterator>(self, f: F) -> Out
    where
        Self: Sized + TreeMap<V, Out, Keys, F>,
        Out: ScopeIter<O> + TreeIter<V>,
    {
        TreeMap::tree_map(self, f)
    }

    fn map<Keys, Out, F, O: Iterator>(self, f: F) -> TreeIteratorScope<U, O, V, Out>
    where
        Self: Sized + Map<V, Out, Keys, F>,
        Out: ScopeIter<O>,
    {
        let tree = self.tree();
        TreeIteratorScope::new(tree, Map::map(self, f))
    }

    /// folds the scoped iter and returns the base iter
    fn into_base_iter(self) -> TreeIterator<V, U>
    where
        Self: Sized + ScopeIter<U>,
    {
        let tree = self.tree();
        TreeIterator::new(tree, self.fold())
    }
}
