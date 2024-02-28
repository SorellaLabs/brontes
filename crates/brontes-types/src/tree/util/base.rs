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

impl<T: Sized + TreeIter<V> + Iterator, V: NormalizedAction> TreeBase<V> for T {}

pub trait TreeIter<V: NormalizedAction> {
    fn tree(&self) -> Arc<BlockTree<V>>;
}

pub struct TreeIterator<V: NormalizedAction, I: Iterator> {
    tree: Arc<BlockTree<V>>,
    iter: I,
}

impl<I: Iterator, V: NormalizedAction> TreeIterator<V, I> {
    pub fn new(tree: Arc<BlockTree<V>>, iter: I) -> Self {
        Self { tree, iter }
    }
}

impl<I: Iterator, V: NormalizedAction> TreeIter<V> for TreeIterator<V, I> {
    fn tree(&self) -> Arc<BlockTree<V>> {
        self.tree.clone()
    }
}

impl<I: Iterator, V: NormalizedAction> Iterator for TreeIterator<V, I> {
    type Item = I::Item;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }
}

/// Base functionality for TreeIter, These are almost all setup or internal
/// tools used to deal with complexity.
pub trait TreeBase<V: NormalizedAction>: Iterator {
    fn dedup<'a, KS, RS, FromI, Out, O, Zip>(
        self,
        parent_actions: KS,
        possible_prune_actions: RS,
    ) -> TreeIterator<V, Out>
    where
        Self: Sized + DedupOperation<'a, FromI, Out, V, Self::Item, Zip> + 'a + TreeIter<V>,
        Out: Iterator,
        V: NormalizedAction + 'a,
        KS: 'a,
        RS: 'a,
        Self: TreeIter<V>,
        FromI: IntoZip<Zip>,
        KS: InTupleFnOutVec<V>,
        RS: InTupleFnOutVec<V>,
        <KS as InTupleFnOutVec<V>>::Out: Dedups<V, RS::Out, FromI, Zip>,
        std::vec::IntoIter<V>: ActionSplit<KS::Out, KS, V> + ActionSplit<RS::Out, RS, V>,
        Zip: SplitIterZip<std::vec::IntoIter<V>>,
    {
        let tree = self.tree();
        TreeIterator::new(tree, DedupOperation::dedup(self, parent_actions, possible_prune_actions))
    }

    fn t_full_map<R, F>(self, f: F) -> R
    where
        Self: Sized + TreeIter<V>,
        F: FnMut((Arc<BlockTree<V>>, Self)) -> R,
    {
        let tree = self.tree();
        Iterator::map(std::iter::once((tree, self)), f)
            .next()
            .unwrap()
    }

    fn t_full_filter_map<R, F>(self, f: F) -> Option<R>
    where
        Self: Sized + TreeIter<V>,
        F: FnMut((Arc<BlockTree<V>>, Self)) -> Option<R>,
    {
        let tree = self.tree();
        Iterator::filter_map(std::iter::once((tree, self)), f).next()
    }

    fn t_map<R, F>(self, f: F) -> TreeIterator<V, std::iter::Map<Self, F>>
    where
        Self: Sized + TreeIter<V>,
        F: FnMut(Self::Item) -> R,
    {
        let tree = self.tree();
        TreeIterator::new(tree, Iterator::map(self, f))
    }

    fn t_filter_map<R, F>(self, f: F) -> TreeIterator<V, FilterMapTree<V, Self, F>>
    where
        Self: Sized + TreeIter<V>,
        F: FnMut(Arc<BlockTree<V>>, Self::Item) -> Option<R>,
    {
        let tree = self.tree();
        TreeIterator::new(tree.clone(), FilterMapTree { tree, iter: self, f })
    }

    fn tree_zip_with<O>(self, other: O) -> TreeIterator<V, Self::Out>
    where
        Self: SplitIterZip<O> + Sized + TreeIter<V>,
        O: Iterator,
    {
        let tree = self.tree();
        TreeIterator::new(tree, SplitIterZip::<O>::zip_with_inner(self, other))
    }

    fn zip_with<O>(self, other: O) -> Self::Out
    where
        Self: SplitIterZip<O> + Sized,
        O: Iterator,
    {
        SplitIterZip::<O>::zip_with_inner(self, other)
    }

    /// Merges the iterator into type O.
    fn merge_iter<O, B>(self) -> TreeIterator<V, B>
    where
        Self: Sized + MergeIter<O, B> + TreeIter<V>,
        B: Iterator<Item = O>,
    {
        let tree = self.tree();
        TreeIterator::new(tree, MergeIter::merge_iter(self))
    }

    /// ensures merge
    fn into_scoped_tree_iter<O, B>(self) -> ScopedIteratorBase<V, B>
    where
        Self: Sized + Iterator + TreeIter<V>,
        Self: Sized + MergeIter<O, B> + TreeIter<V>,
        B: Iterator<Item = O>,
    {
        let this = TreeBase::merge_iter(self);
        ScopedIteratorBase::new(this.tree(), this.iter)
    }
}

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
