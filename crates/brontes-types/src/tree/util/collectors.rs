use crate::{
    normalized_actions::NormalizedAction, ActionSplit, MergeInto, MergeIntoUnpadded, TreeBase,
    UnzipPadded,
};

impl<T: Sized + TreeBase<V>, V: NormalizedAction> TreeCollector<V> for T {}

/// All Collection functionality on the TreeIters
pub trait TreeCollector<V: NormalizedAction>: TreeBase<V> {
    fn split_actions<FromI, Fns>(self, filters: Fns) -> FromI
    where
        Self: Sized + ActionSplit<FromI, Fns, V>,
    {
        ActionSplit::action_split_impl(self, filters)
    }

    fn split_actions_ref<FromI, Fns>(self, filters: &Fns) -> FromI
    where
        Self: Sized + ActionSplit<FromI, Fns, V>,
    {
        ActionSplit::action_split_ref_impl(self, filters)
    }

    fn split_return_rem<FromI, Fns>(self, filters: Fns) -> (FromI, Vec<V>)
    where
        Self: Sized + ActionSplit<FromI, Fns, V>,
        FromI: IntoIterator,
    {
        ActionSplit::action_split_out_impl(self, filters)
    }

    fn split_return_ref_rem<FromI, Fns>(self, filters: &Fns) -> (FromI, Vec<V>)
    where
        Self: Sized + ActionSplit<FromI, Fns, V>,
        FromI: IntoIterator,
    {
        ActionSplit::action_split_out_ref_impl(self, filters)
    }

    fn unzip_padded<FromZ>(self) -> FromZ
    where
        Self: UnzipPadded<FromZ> + Sized,
    {
        UnzipPadded::unzip_padded(self)
    }

    fn merge_into<I, Ty>(self) -> I
    where
        Self: MergeInto<I, Ty, Self::Item> + Sized,
    {
        MergeInto::merge_into(self)
    }

    fn merge_into_unpadded<I, Ty>(self) -> I
    where
        Self: MergeIntoUnpadded<I, Ty, Self::Item> + Sized,
    {
        MergeIntoUnpadded::merge_into_unpadded(self)
    }
}
