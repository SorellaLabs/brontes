use std::{iter::Iterator, sync::Arc};

use super::{
    DedupOperation, Dedups, FlattenSpecified, InTupleFnOutVec, Map, SplitIterZip,
    TreeIteratorScope, TreeMap,
};
use crate::{
    normalized_actions::{NormalizedAction, NormalizedMint, NormalizedSwap},
    BlockTree, Filter, FilterTree, MergeIter, ScopeIter, ScopedIteratorBase,
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
    fn new(tree: Arc<BlockTree<V>>, iter: I) -> Self {
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
pub trait TreeBase<V: NormalizedAction>: TreeIter<V> + Iterator {
    fn dedup<KS, RS, FromI, Out>(
        self,
        parent_actions: KS,
        possible_prune_actions: RS,
    ) -> TreeIterator<V, Out::IntoIter>
    where
        Out: IntoIterator,
        Self: Sized + DedupOperation<FromI, Out, V, Self::Item>,
        FromI: IntoIterator,
        KS: InTupleFnOutVec<V>,
        <KS as InTupleFnOutVec<V>>::Out: Dedups<RS::Out, FromI>,
        RS: InTupleFnOutVec<V>,
    {
        let tree = self.tree();
        TreeIterator::new(
            tree,
            DedupOperation::dedup(self, parent_actions, possible_prune_actions).into_iter(),
        )
    }

    fn zip_with<O>(self, other: O) -> TreeIterator<V, Self::Out>
    where
        Self: SplitIterZip<O> + Sized,
        O: Iterator,
    {
        let tree = self.tree();
        TreeIterator::new(tree, SplitIterZip::<O>::zip_with_inner(self, other))
    }

    fn flatten_specific<R, W, T>(
        self,
        wanted: W,
        transform: T,
    ) -> TreeIterator<V, FlattenSpecified<V, Self, W, T>>
    where
        Self: Sized,
        Self: ScopeIter<V>,
        R: Clone,
        W: Fn(&V) -> Option<&R>,
        T: Fn(R) -> Vec<V>,
    {
        let tree = self.tree();
        TreeIterator::new(tree, FlattenSpecified::new(self, wanted, transform))
    }

    /// Merges the iterator into type O.
    fn merge_iter<O, B>(self) -> TreeIterator<V, B>
    where
        Self: Sized + MergeIter<O, B>,
        B: Iterator<Item = O>,
    {
        let tree = self.tree();
        TreeIterator::new(tree, MergeIter::merge_iter(self))
    }

    fn into_scoped_tree_iter(self) -> ScopedIteratorBase<V, Self>
    where
        Self: Sized + Iterator,
    {
        ScopedIteratorBase::new(self.tree(), self)
    }
}

impl<V: NormalizedAction, T: TreeIter<V> + ScopeIter<V>> TreeScoped<V> for T {}

pub trait TreeScoped<V: NormalizedAction>: TreeIter<V> + ScopeIter<V> {
    fn filter_with_tree<Out, Keys, F>(self, keys: Keys, f: F) -> Out
    where
        Self: Sized + FilterTree<V, Out, Keys, F>,
        Out: ScopeIter<V> + TreeIter<V>,
    {
        FilterTree::filter(self, keys, f)
    }

    fn filter<Out, Keys, F>(self, keys: Keys, f: F) -> TreeIteratorScope<V, Out>
    where
        Self: Sized + Filter<V, Out, Keys, F>,
        Out: ScopeIter<V>,
        F: FnMut(Keys) -> bool,
    {
        let tree = self.tree();
        TreeIterator::new(tree, Filter::filter(self, keys, f))
    }

    fn map_tree_map<Out, Keys, F>(self, keys: Keys, f: F) -> Out
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
        TreeIterator::new(tree, Map::map(self, keys, f))
    }
}

fn test() {
    use crate::normalized_actions::Actions;
    let iter: Vec<(NormalizedSwap, NormalizedMint)> = vec![];
    let tree: Arc<BlockTree<Actions>> = Arc::new(BlockTree::new(Default::default(), 69));
    let tree_iter = TreeIterator::new(tree, iter.into_iter());
    let a = tree_iter.into_scoped_tree_iter();
}
