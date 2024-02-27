use std::sync::Arc;

use super::{
    DedupOperation, Dedups, FlattenSpecified, InTupleFnOutVec, MergeInto, MergeIntoUnpadded,
    SplitIterZip,
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

    fn split_actions<FromI, Fns>(self, filters: Fns) -> TreeIterator<V, FromI::IntoIter>
    where
        Self: Sized + ActionSplit<FromI, Fns, V, Self::Item>,
        FromI: IntoIterator,
    {
        let tree = self.tree();
        TreeIterator::new(tree, ActionSplit::action_split_impl(self, filters).into_iter())
    }

    fn split_actions_ref<FromI, Fns>(self, filters: &Fns) -> TreeIterator<V, FromI::IntoIter>
    where
        Self: Sized + ActionSplit<FromI, Fns, V, Self::Item>,
        FromI: IntoIterator,
    {
        let tree = self.tree();
        TreeIterator::new(tree, ActionSplit::action_split_ref_impl(self, filters).into_iter())
    }

    fn split_return_rem<FromI, Fns>(self, filters: Fns) -> (FromI, Vec<V>)
    where
        Self: Sized + ActionSplit<FromI, Fns, V, Self::Item>,
        FromI: IntoIterator,
    {
        ActionSplit::action_split_out_impl(self, filters)
    }

    fn split_return_ref_rem<FromI, Fns>(self, &filters: Fns) -> (FromI, Vec<V>)
    where
        Self: Sized + ActionSplit<FromI, Fns, V, Self::Item>,
        FromI: IntoIterator,
    {
        ActionSplit::action_split_out_ref_impl(self, filters)
    }

    fn zip_with<O>(self, other: O) -> TreeIterator<V, Self::Out>
    where
        Self: SplitIterZip<O> + Sized,
        O: Iterator,
    {
        let tree = self.tree();
        TreeIterator::new(tree, SplitIterZip::<O>::zip_with_inner(self, other))
    }

    fn unzip_padded<FromZ>(self) -> FromZ
    where
        Self: UnzipPadded<FromZ> + Sized,
    {
        UnzipPadded::unzip_padded(self)
    }

    fn merge_into<I, Ty>(self) -> TreeIterator<V, I::IntoIter>
    where
        Self: MergeInto<I, Ty, Self::Item> + Sized,
        I: IntoIterator,
    {
        let tree = self.tree();
        TreeIterator::new(tree, MergeInto::merge_into(self).into_iter())
    }

    fn merge_into_unpadded<I, Ty>(self) -> TreeIterator<V, I::IntoIter>
    where
        Self: MergeIntoUnpadded<I, Ty, Self::Item> + Sized,
        I: IntoIterator,
    {
        let tree = self.tree();
        TreeIterator::new(tree, MergeIntoUnpadded::merge_into_unpadded(self).into_iter())
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

    /// Allows to change what actions are being looked at
    fn change_map_scope(self) {}

    fn filter_all(self) {}
    fn filter_all_with_tree(self) {}

    fn filter_specific(self) {}
    fn filter_specific_with_tree(self) {}

    fn map_all(self) {}
    fn map_all_with_tree(self) {}

    fn map_specific(self) {}
    fn map_specific_with_tree(self) {}

    fn map_types(self) {}
    fn map_types_with_tree(self) {}

    fn count_action(self) {}
    fn count_actions(self) {}
}
