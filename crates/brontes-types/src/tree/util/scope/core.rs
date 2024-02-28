use std::{
    iter::{Iterator, Once},
    sync::Arc,
};

use super::{
    DedupOperation, Dedups, InTupleFnOutVec, Map, SplitIterZip, TreeIteratorScope, TreeMap,
};
use crate::{
    normalized_actions::NormalizedAction, ActionSplit, BlockTree, Filter, FilterMapTree,
    FilterTree, IntoZip, IntoZippedIter, MergeIter, ScopeIter, ScopedIteratorBase,
};

impl<V: NormalizedAction, T: TreeIter<V> + ScopeIter<V>> TreeScoped<V> for T where T: Sized {}

pub trait TreeScoped<V: NormalizedAction>: TreeIter<V> + ScopeIter<V> {
    fn filter_with_tree<Out, Keys, F>(self, keys: Keys, f: F) -> Out
    where
        Self: Sized + FilterTree<V, Out, Keys, F>,
        Out: ScopeIter<V> + TreeIter<V>,
    {
        FilterTree::filter_tree(self, keys, f)
    }

    fn filter<Out, Keys, F>(self, keys: Keys, f: F) -> TreeIteratorScope<V, Out>
    where
        Self: Sized + Filter<V, Out, Keys, F>,
        Out: ScopeIter<V>,
        F: FnMut(Keys) -> bool,
    {
        let tree = self.tree();
        TreeIteratorScope::new(tree, Filter::filter(self, keys, f))
    }

    fn tree_map<Out, Keys, F>(self, keys: Keys, f: F) -> Out
    where
        Self: Sized + TreeMap<V, Out, Keys, F>,
        Out: ScopeIter<V> + TreeIter<V>,
    {
        TreeMap::tree_map(self, keys, f)
    }

    fn map<Out, Keys, F>(self, keys: Keys, f: F) -> TreeIteratorScope<V, Out>
    where
        Self: Sized + Map<V, Out, Keys, F>,
        Out: ScopeIter<V>,
    {
        let tree = self.tree();
        TreeIteratorScope::new(tree, Map::map(self, keys, f))
    }
}
