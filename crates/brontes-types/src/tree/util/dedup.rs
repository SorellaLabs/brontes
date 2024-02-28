use alloy_primitives::B256;

use super::InTupleFnOutVec;
use crate::{
    action_iter::ActionIter,
    normalized_actions::{utils::ActionCmp, NormalizedAction},
    ActionSplit, IntoZippedIter, SplitIterZip, TreeBase, TreeIter, TreeIterator,
};

pub trait DedupOperation<'a, FromI, Out, V: NormalizedAction, Item, ZIP> {
    fn dedup<KS, RS>(self, parent_actions: KS, possible_prune_actions: RS) -> Out
    where
        V: NormalizedAction + 'a,
        KS: 'a,
        RS: 'a,
        Self: TreeIter<V>,
        FromI: IntoZippedIter<IntoIter = ZIP>,
        KS: InTupleFnOutVec<V>,
        RS: InTupleFnOutVec<V>,
        <KS as InTupleFnOutVec<V>>::Out: Dedups<V, RS::Out, FromI>,
        std::vec::IntoIter<V>: ActionSplit<KS::Out, KS, V> + ActionSplit<RS::Out, RS, V>,
        ZIP: SplitIterZip<std::vec::IntoIter<V>>;
}

pub struct Deduped<'a, I> {
    iterator: Box<dyn Iterator<Item = I> + 'a>,
}

impl<I> Iterator for Deduped<'_, I> {
    type Item = I;

    fn next(&mut self) -> Option<Self::Item> {
        self.iterator.next()
    }
}

/// Collect All Impl
impl<'a, T, FromI: IntoIterator, V: NormalizedAction, FUCK>
    DedupOperation<
        'a,
        FromI,
        Deduped<'a, (B256, <FUCK as SplitIterZip<std::vec::IntoIter<V>>>::Out)>,
        V,
        (B256, Vec<V>),
        FUCK,
    > for T
where
    T: 'a,
    T: Iterator<Item = (B256, Vec<V>)> + TreeIter<V>,
    TreeIterator<V, <FromI as IntoIterator>::IntoIter>: SplitIterZip<std::vec::IntoIter<V>>,
    FUCK: SplitIterZip<std::vec::IntoIter<V>> + TreeBase<V>,
{
    fn dedup<KS, RS>(
        self,
        parent_actions: KS,
        possible_prune_actions: RS,
    ) -> Deduped<'a, (B256, <FUCK as SplitIterZip<std::vec::IntoIter<V>>>::Out)>
    where
        V: NormalizedAction + 'a,
        KS: 'a,
        RS: 'a,
        Self: Iterator<Item = (B256, Vec<V>)> + TreeIter<V>,
        KS: InTupleFnOutVec<V>,
        RS: InTupleFnOutVec<V>,
        <KS as InTupleFnOutVec<V>>::Out: Dedups<V, RS::Out, FromI>,
        std::vec::IntoIter<V>: ActionSplit<KS::Out, KS, V> + ActionSplit<RS::Out, RS, V>,
        FUCK: SplitIterZip<std::vec::IntoIter<V>>,
        FromI: IntoZippedIter<IntoIter = FUCK>,
    {
        Deduped {
            iterator: Box::new(self.map(move |(k, v)| {
                let (good, rem): (KS::Out, Vec<V>) =
                    v.into_iter().action_split_out_ref(&parent_actions);

                let (bad, rem): (RS::Out, Vec<V>) = rem
                    .into_iter()
                    .action_split_out_ref(&possible_prune_actions);

                let res = good
                    .merge_removing_duplicates(bad)
                    .into_zipped_iter()
                    .zip_with(rem.into_iter());

                (k, res)
            })),
        }
    }
}

// collect Some impl
// impl<T, FromI, V: NormalizedAction>
//     DedupOperation<
//         FromI,
//         Deduped<<<FromI as IntoIterator>::IntoIter as
// SplitIterZip<std::vec::IntoIter<V>>>::Out>,         V,
//         Vec<V>,
//     > for T
// where
//     T: Iterator<Item = Vec<V>>,
//     FromI: IntoIterator,
//     <FromI as IntoIterator>::IntoIter: SplitIterZip<std::vec::IntoIter<V>>,
// {
//     fn dedup<KS, RS>(
//         self,
//         parent_actions: KS,
//         possible_prune_actions: RS,
//     ) -> Deduped<<<FromI as IntoIterator>::IntoIter as
// SplitIterZip<std::vec::IntoIter<V>>>::Out>     where
//         FromI: IntoTreeIterator,
//         V: NormalizedAction,
//         KS: InTupleFnOutVec<V>,
//         <KS as InTupleFnOutVec<V>>::Out: Dedups<RS::Out, FromI>,
//         <FromI as IntoIterator>::IntoIter:
// SplitIterZip<std::vec::IntoIter<V>>,         RS: InTupleFnOutVec<V>,
//         std::vec::IntoIter<V>:
//             ActionSplit<KS::Out, KS, V, Vec<V>> + ActionSplit<RS::Out, RS, V,
// Vec<V>>,     {
//         Deduped {
//             iterator: Box::new(self.map(|v| {
//                 let (good, rem): (KS::Out, Vec<V>) =
//                     v.into_iter().action_split_out_ref(&parent_actions);
//
//                 let (bad, rem): (RS::Out, Vec<V>) = rem
//                     .into_iter()
//                     .action_split_out_ref(&possible_prune_actions);
//
//                 let merged = good.merge_removing_duplicates(bad);
//                 merged.into_split_iter().zip_with(rem.into_iter())
//             })),
//         }
//     }
// }
//
// // collect one
// impl<T, FromI, V: NormalizedAction>
//     DedupOperation<
//         FromI,
//         <<FromI as IntoIterator>::IntoIter as
// SplitIterZip<std::vec::IntoIter<V>>>::Out,         V,
//         V,
//     > for T
// where
//     T: Iterator<Item = V>,
//     FromI: IntoIterator,
//     <FromI as IntoIterator>::IntoIter: SplitIterZip<std::vec::IntoIter<V>>,
// {
//     fn dedup<KS, RS>(
//         self,
//         parent_actions: KS,
//         possible_prune_actions: RS,
//     ) -> <<FromI as IntoIterator>::IntoIter as
// SplitIterZip<std::vec::IntoIter<V>>>::Out     where
//         FromI: IntoIterator,
//         V: NormalizedAction,
//         KS: InTupleFnOutVec<V>,
//         <KS as InTupleFnOutVec<V>>::Out: Dedups<RS::Out, FromI>,
//         <FromI as IntoIterator>::IntoIter:
// SplitIterZip<std::vec::IntoIter<V>>,         RS: InTupleFnOutVec<V>,
//         std::vec::IntoIter<V>: ActionSplit<KS::Out, KS, V, V> +
// ActionSplit<RS::Out, RS, V, V>,     {
//         let (good, rem): (KS::Out, Vec<V>) =
// self.action_split_out_ref(&parent_actions);
//
//         let (bad, rem): (RS::Out, Vec<V>) = rem
//             .into_iter()
//             .action_split_out_ref(&possible_prune_actions);
//
//         let merged = good.merge_removing_duplicates(bad);
//         merged.into_split_iter().zip_with(rem.into_iter())
//     }
// }

pub trait Dedups<V: NormalizedAction, RI, FromI>: IntoIterator {
    /// Given the current iterator, or tuple of iterators, merges them and
    /// and then dedups the other iterators
    fn merge_removing_duplicates(self, merge_dedup_iters: RI) -> FromI
    where
        FromI: IntoZippedIter;
}

macro_rules! tree_dedup {
    ($((
                $([
                  $remove_i:ident,
                  $remove_type:ident,
                  $ret_r: ident
                ],)*
                $keep_type:ident,
                $ret_k:ident
    )),*) => {
        paste::paste!(
        impl <
            K,
            V: NormalizedAction,
            $($($remove_i: IntoIterator<Item = $remove_type> + Clone,)*)*
            $($($remove_type: PartialEq + Eq,)*)*
            $($keep_type: $(ActionCmp<$remove_type> + )*,)*
            $($($ret_r: Default + Extend<$remove_type>,)*)*
            $($ret_k: Default + Extend<$keep_type>,)*
            >
            Dedups
            <
            V,
            ($($($remove_i),*),*),
            ($($ret_k),*, $($($ret_r),*),*)
            > for K
            where
                K: IntoIterator<Item = ($($keep_type),*)>,
                ($($($ret_r),*),*, $($ret_k),*): IntoZippedIter,
            {
                #[allow(non_snake_case, unused_variables, unused_mut)]
                fn merge_removing_duplicates(self, remove_i: ($($($remove_i),*),*))
                    -> ($($ret_k),*, $($($ret_r),*),*) {

                    let ($($(mut $ret_r,)*)*) = ($($($ret_r::default(),)*)*);

                    let ($(mut $ret_k,)*) = ($($ret_k::default(),)*);

                    $($(
                        let mut [<$ret_r _filtered>] = vec![];
                    )*)*

                    let ($($($remove_i),*),*) = remove_i;

                    self.into_iter().for_each(|($($keep_type),*)| {
                        $(
                            $(
                                 let cloned_iter = $remove_i.clone();
                                 for c_entry in cloned_iter.into_iter(){
                                    if $keep_type.is_superior_action(&c_entry) {
                                        [<$ret_r _filtered>].push(c_entry);
                                     }
                                  }
                              )*
                            $ret_k.extend(std::iter::once($keep_type));

                        )*
                    });

                    $(
                        $(
                            for i in $remove_i {
                                if ![<$ret_r _filtered>].contains(&i) {
                                    $ret_r.extend(std::iter::once(i));
                                }
                            }
                        )*
                     )*

                    ($($ret_k),*,$($($ret_r),*),*)
                }
            }
        );
    };
}

tree_dedup!(([RI0, RT0, RR0], KT0, KK0));
tree_dedup!(([RI0, RT0, RR0], [RI1, RT1, RR1], KT0, KK0));
tree_dedup!(([RI0, RT0, RR0], [RI1, RT1, RR1], [RI2, RT2, RR2], KT0, KK0));

tree_dedup!(([RI0, RT0, RR0], KT0, KK0), ([RI1, RT1, RR1], KT1, KK1));
tree_dedup!(([RI0, RT0, RR0], KT0, KK0), ([RI1, RT1, RR1], KT1, KK1), ([RI2, RT2, RR2], KT2, KK2));
