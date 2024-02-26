use alloy_primitives::B256;

use crate::{
    normalized_actions::{utils::ActionCmp, NormalizedAction, NormalizedSwap},
    ActionIter, ActionSplit, BlockTree, IntoSplitIterator, TreeSearchBuilder,
};

pub trait TreeFilter<V: NormalizedAction> {
    fn collect_all_deduping<'a, KS, RS, KF, RF, FromI>(
        &self,
        call: TreeSearchBuilder<V>,
        k_split: KS,
        r_split: RS,
    ) -> impl Iterator<Item = (B256, FromI)> + 'a
    where
        Self: Dedups<RS::Out, FromI>,
        KS: InTupleFnOutVec<V>,
        RS: InTupleFnOutVec<V>,
        std::vec::IntoIter<V>: ActionSplit<KS::Out, KS, V> + ActionSplit<RS::Out, RS, V>,
        <KS as InTupleFnOutVec<V>>::Out: IntoSplitIterator,
        <RS as InTupleFnOutVec<V>>::Out: IntoSplitIterator,
        FromI: IntoSplitIterator,
        KS: 'a,
        RS: 'a,
        V: 'a;

    fn collect_txes_deduping<'a, KS, RS, KF, RF>(
        &'a self,
        txes: &'a [B256],
        call: TreeSearchBuilder<V>,
        k_split: KS,
        r_split: RS,
    ) -> impl Iterator<Item = Vec<V>> + 'a
    where
        Self: Dedups<RS::Out, FromI>,
        KS: InTupleFnOutVec<V>,
        RS: InTupleFnOutVec<V>,
        std::vec::IntoIter<V>: ActionSplit<KS::Out, KS, V> + ActionSplit<RS::Out, RS, V>,
        KS: 'a,
        RS: 'a,
        V: 'a;

    fn collect_tx_deduping<'a, KS, RS, KF, RF>(
        &'a self,
        tx: &'a B256,
        call: TreeSearchBuilder<V>,
        k_split: KS,
        r_split: RS,
    ) -> impl Iterator<Item = V> + 'a
    where
        Self: Dedups<V, KS::Out, RS::Out, KF, RF>,
        KS: InTupleFnOutVec<V>,
        RS: InTupleFnOutVec<V>,
        std::vec::IntoIter<V>: ActionSplit<KS::Out, KS, V> + ActionSplit<RS::Out, RS, V>,
        KS: 'a,
        RS: 'a,
        V: 'a;
}

impl<V: NormalizedAction> TreeFilter<V> for BlockTree<V> {
    fn collect_all_deduping<'a, KS, RS, KF, RF, FromI>(
        &self,
        call: TreeSearchBuilder<V>,
        k_split: KS,
        r_split: RS,
    ) -> impl Iterator<Item = (B256, FromI)> + 'a
    where
        Self: Dedups<RS::Out, FromI>,
        KS: InTupleFnOutVec<V>,
        RS: InTupleFnOutVec<V>,
        std::vec::IntoIter<V>: ActionSplit<KS::Out, KS, V> + ActionSplit<RS::Out, RS, V>,
        FromI: IntoSplitIterator,
        KS: 'a,
        <KS as InTupleFnOutVec<V>>::Out: IntoSplitIterator,
        <RS as InTupleFnOutVec<V>>::Out: IntoSplitIterator,
        RS: 'a,
        V: 'a,
    {
        self.collect_all(call).into_iter().map(move |(k, v)| {
            let (good, mut rem) = v.clone().into_iter().action_split_out_ref(&k_split);
            let bad = v.into_iter().action_split_ref(&r_split);

            let a = good.merge_removing_duplicates(bad);
            // rem.extend(Self::dedup_action_vec(good, bad));
            // rem.sort_by_key(|k| k.get_trace_index());

            (k, a)
        })
    }

    fn collect_txes_deduping<'a, KS, RS, KF, RF>(
        &'a self,
        txes: &'a [B256],
        call: TreeSearchBuilder<V>,
        k_split: KS,
        r_split: RS,
    ) -> impl Iterator<Item = Vec<V>> + 'a
    where
        Self: Dedups<V, KS::Out, RS::Out, KF, RF>,
        KS: InTupleFnOutVec<V>,
        RS: InTupleFnOutVec<V>,
        std::vec::IntoIter<V>: ActionSplit<KS::Out, KS, V> + ActionSplit<RS::Out, RS, V>,
        KS: 'a,
        RS: 'a,
        V: 'a,
    {
        self.collect_txes(txes, call).into_iter().map(move |v| {
            let (good, mut rem) = v.clone().into_iter().action_split_out_ref(&k_split);
            let bad = v.into_iter().action_split_ref(&r_split);

            rem.extend(Self::dedup_action_vec(good, bad));
            rem.sort_by_key(|k| k.get_trace_index());

            rem
        })
    }

    fn collect_tx_deduping<'a, KS, RS, KF, RF>(
        &'a self,
        tx: &'a B256,
        call: TreeSearchBuilder<V>,
        k_split: KS,
        r_split: RS,
    ) -> impl Iterator<Item = V> + 'a
    where
        Self: Dedups<V, KS::Out, RS::Out, KF, RF>,
        KS: InTupleFnOutVec<V>,
        RS: InTupleFnOutVec<V>,
        std::vec::IntoIter<V>: ActionSplit<KS::Out, KS, V> + ActionSplit<RS::Out, RS, V>,

        KS: 'a,
        RS: 'a,
        V: 'a,
    {
        let v = self.collect(tx, call);

        let (good, rem) = v.into_iter().action_split_out_ref(&k_split);
        let (bad, mut rem) = rem.into_iter().action_split_out_ref(&r_split);

        rem.extend(Self::dedup_action_vec(good, bad));
        rem.sort_by_key(|k| k.get_trace_index());

        rem.into_iter()
    }
}

pub trait Dedups<RI, FromI>: IntoSplitIterator {
    /// Given the current iterator, or tuple of iterators, merges them and
    /// and then dedups the other iterators
    fn merge_removing_duplicates(self, merge_dedup_iters: RI) -> FromI
    where
        FromI: IntoSplitIterator;
}

fn test() {
    use crate::normalized_actions::Actions;
    let v1: (Vec<NormalizedSwap>, Vec<NormalizedSwap>) = (vec![], vec![]);
    let ve: Vec<Actions> = vec![];
    // v1.merge_removing_duplicates(merge_dedup_iters)
}

macro_rules! tree_dedup {
    ($((
                $keep_i:ident,
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
            $($keep_i: IntoIterator<Item = $keep_type>,)*
            $($($remove_i: IntoIterator<Item = $remove_type> + Clone,)*)*
            $($($remove_type: PartialEq + Eq,)*)*
            $($keep_type: $(ActionCmp<$remove_type> + )*,)*
            $($($ret_r: Default + Extend<$remove_type>,)*)*
            $($ret_k: Default + Extend<$keep_type>,)*
            >
            Dedups
            <
            ($($($remove_i,)*)*),
            // ($($keep_type,)*),
            // ($($($remove_type,)*)*),
            ($($($ret_r,)*)* $($ret_k,)*)
            > for ($($keep_i,)*)
            where
                ($($keep_i,)*): IntoSplitIterator<Item = ($(Option<$keep_type>,)*)>,
                ($($($ret_r,)*)* $($ret_k,)*): IntoSplitIterator,
            {
                #[allow(non_snake_case, unused_variables, unused_mut)]
                fn merge_removing_duplicates(self, remove_i: ($($($remove_i,)*)*))
                    -> ($($($ret_r,)*)* $($ret_k,)*) {

                    let ($($(mut $ret_r,)*)*) = ($($($ret_r::default(),)*)*);
                    let ($(mut $ret_k,)*) = ($($ret_k::default(),)*);

                    $($(
                        let mut [<$ret_r _filtered>] = vec![];
                    )*)*

                    let ($($($remove_i,)*)*) = remove_i;

                    self.into_split_iter().for_each(|($($keep_type,)*)| {
                        $(
                            if let Some(keep) = $keep_type {
                                $(
                                     let cloned_iter = $remove_i.clone();
                                     for c_entry in cloned_iter.into_iter(){
                                        if keep.is_superior_action(&c_entry) {
                                            [<$ret_r _filtered>].push(c_entry);
                                         }
                                      }
                                  )*
                                $ret_k.extend(std::iter::once(keep));
                            }

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

                    ($($($ret_r,)*)* $($ret_k,)*)
                }
            }
        );
    };
}

tree_dedup!((KI0, [RI0, RT0, RR0], KT0, KK0));
tree_dedup!((KI0, [RI0, RT0, RR0], KT0, KK0), (KI1, [RI1, RT1, RR1], KT1, KK1));
tree_dedup!(
    (KI0, [RI0, RT0, RR0], KT0, KK0),
    (KI1, [RI1, RT1, RR1], KT1, KK1),
    (KI2, [RI2, RT2, RR2], KT2, KK2)
);
// tree_dedup!(
//     (KI0, [RI0, RT0], KT0),
//     (KI1, [RI1, RT1], KT1),
//     (KI2, [RI2, RT2], KT2),
//     (KI3, [RI3, RT3], KT3)
// );
// tree_dedup!(
//     (KI0, [RI0, RT0], KT0),
//     (KI1, [RI1, RT1], KT1),
//     (KI2, [RI2, RT2], KT2),
//     (KI3, [RI3, RT3], KT3),
//     (KI4, [RI4, RT4], KT4)
// );

pub trait InTupleFnOutVec<V: NormalizedAction> {
    type Out: ActionCmp;
}

macro_rules! in_tuple_out_vec {
    ($($out:ident),*) => {
        impl<V: NormalizedAction, $($out,)*> InTupleFnOutVec<V>
            for ($( Box<dyn Fn(V) -> Option<$out>>,)*) {
            type Out = ($( Vec<$out>,)*);
        }
    };
}

in_tuple_out_vec!(T0);
in_tuple_out_vec!(T0, T1);
in_tuple_out_vec!(T0, T1, T2);
in_tuple_out_vec!(T0, T1, T2, T3);
in_tuple_out_vec!(T0, T1, T2, T3, T4);
in_tuple_out_vec!(T0, T1, T2, T3, T4, T5);
in_tuple_out_vec!(T0, T1, T2, T3, T4, T5, T6);
