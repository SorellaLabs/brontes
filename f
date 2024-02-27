use std::{iter::Map, sync::Arc};

use super::{
    DedupOperation, Dedups, FlattenSpecified, InTupleFnOutVec, MergeInto, MergeIntoUnpadded,
    SplitIterZip, TreeMap,
};
use crate::{normalized_actions::NormalizedAction, ActionSplit, BlockTree, UnzipPadded};

impl<T: Sized + TreeIter<V>, V: NormalizedAction> TreeBase<V> for T {}

pub trait TreeIter<V: NormalizedAction>: Iterator {
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

/// Base Actions for TreeIter, These are almost all setup or internal tools used
/// to deal with complexity.
pub trait TreeBase<V: NormalizedAction>: TreeIter<V> {
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
        R: Clone,
        W: Fn(&V) -> Option<&R>,
        T: Fn(R) -> Vec<V>,
    {
        let tree = self.tree();
        TreeIterator::new(tree, FlattenSpecified::new(self, wanted, transform))
    }

    /// changes what items of the underlying selected actions are put through
    /// the iter
    fn change_iter_scope(self) {}

    fn filter(self) {}
    fn filter_with_tree(self) {}

    fn map<B, F>(self, f: F) -> TreeIterator<V, Map<Self, F>>
    where
        Self: Sized,
        F: FnMut(Self::Item) -> B,
    {
        let tree = self.tree();
        TreeIterator::new(tree, self.map(f))
    }

    fn map_with_tree<B, F>(self, f: F) -> TreeMap<V, Self, F>
    where
        Self: Sized,
        F: FnMut(Self::Item) -> B,
    {
        TreeMap::new(self.tree(), self, f)
    }

    fn count_action(self) {}
    fn count_actions(self) {}
}
