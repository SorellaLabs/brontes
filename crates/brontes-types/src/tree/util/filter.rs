use std::{collections::HashMap, iter::Iterator};

use alloy_primitives::B256;

use crate::{
    normalized_actions::{utils::ActionCmp, NormalizedAction},
    ActionIter, ActionSplit, BlockTree, TreeSearchBuilder,
};

pub trait TreeFilter<V: NormalizedAction> {
    fn collect_all_deduping<KS, RS, KF, RF>(
        &self,
        call: TreeSearchBuilder<V>,
        k_split: KS,
        r_split: RS,
    ) -> HashMap<B256, Vec<V>>
    where
        Self: TreeDedup<V, KS::Out, RS::Out, KF, RF>,
        KS: InTupleFnOutVec<V>,
        RS: InTupleFnOutVec<V>,
        std::vec::IntoIter<V>: ActionSplit<KS::Out, KS, V> + ActionSplit<RS::Out, RS, V>;

    fn collect_txes_deduping<KS, RS, KF, RF>(
        &self,
        txes: Vec<B256>,
        call: TreeSearchBuilder<V>,
        k_split: KS,
        r_split: RS,
    ) -> HashMap<B256, Vec<V>>
    where
        Self: TreeDedup<V, KS::Out, RS::Out, KF, RF>,
        KS: InTupleFnOutVec<V>,
        RS: InTupleFnOutVec<V>,
        std::vec::IntoIter<V>: ActionSplit<KS::Out, KS, V> + ActionSplit<RS::Out, RS, V>;

    fn collect_tx_deduping<KS, RS, KF, RF>(
        &self,
        tx: B256,
        call: TreeSearchBuilder<V>,
        k_split: KS,
        r_split: RS,
    ) -> Vec<V>
    where
        Self: TreeDedup<V, KS::Out, RS::Out, KF, RF>,
        KS: InTupleFnOutVec<V>,
        RS: InTupleFnOutVec<V>,
        std::vec::IntoIter<V>: ActionSplit<KS::Out, KS, V> + ActionSplit<RS::Out, RS, V>;
}

impl<V: NormalizedAction> TreeFilter<V> for BlockTree<V> {
    fn collect_all_deduping<KS, RS, KF, RF>(
        &self,
        call: TreeSearchBuilder<V>,
        k_split: KS,
        r_split: RS,
    ) -> HashMap<B256, Vec<V>>
    where
        Self: TreeDedup<V, KS::Out, RS::Out, KF, RF>,
        KS: InTupleFnOutVec<V>,
        RS: InTupleFnOutVec<V>,
        std::vec::IntoIter<V>: ActionSplit<KS::Out, KS, V> + ActionSplit<RS::Out, RS, V>,
    {
        self.collect_all(call)
            .into_iter()
            .map(|(k, v)| {
                let (good, mut rem) = v.clone().into_iter().action_split_out_ref(&k_split);
                let bad = v.into_iter().action_split_ref(&r_split);

                rem.extend(Self::dedup_action_vec(good, bad));
                rem.sort_by_key(|k| k.get_trace_index());

                (k, rem)
            })
            .collect()
    }

    fn collect_txes_deduping<KS, RS, KF, RF>(
        &self,
        txes: Vec<B256>,
        call: TreeSearchBuilder<V>,
        k_split: KS,
        r_split: RS,
    ) -> HashMap<B256, Vec<V>>
    where
        Self: TreeDedup<V, KS::Out, RS::Out, KF, RF>,
        KS: InTupleFnOutVec<V>,
        RS: InTupleFnOutVec<V>,
        std::vec::IntoIter<V>: ActionSplit<KS::Out, KS, V> + ActionSplit<RS::Out, RS, V>,
    {
        self.collect_txes(txes, call)
            .into_iter()
            .map(|(k, v)| {
                let (good, mut rem) = v.clone().into_iter().action_split_out_ref(&k_split);
                let bad = v.into_iter().action_split_ref(&r_split);

                rem.extend(Self::dedup_action_vec(good, bad));
                rem.sort_by_key(|k| k.get_trace_index());

                (k, rem)
            })
            .collect()
    }

    fn collect_tx_deduping<KS, RS, KF, RF>(
        &self,
        tx: B256,
        call: TreeSearchBuilder<V>,
        k_split: KS,
        r_split: RS,
    ) -> Vec<V>
    where
        Self: TreeDedup<V, KS::Out, RS::Out, KF, RF>,
        KS: InTupleFnOutVec<V>,
        RS: InTupleFnOutVec<V>,
        std::vec::IntoIter<V>: ActionSplit<KS::Out, KS, V> + ActionSplit<RS::Out, RS, V>,
    {
        let v = self.collect(tx, call);

        let (good, mut rem) = v.clone().into_iter().action_split_out_ref(&k_split);
        let bad = v.into_iter().action_split_ref(&r_split);

        rem.extend(Self::dedup_action_vec(good, bad));
        rem.sort_by_key(|k| k.get_trace_index());

        rem
    }
}

pub trait TreeDedup<V: NormalizedAction, KI, RI, KT, RT> {
    fn dedup_action_vec(keep_i: KI, remove_i: RI) -> Vec<V>;
}

macro_rules! tree_dedup {
    ($((
                $keep_i:ident,
                $([
                  $remove_i:ident,
                  $remove_type:ident
                ],)*
                $keep_type:ident
    )),*) => {
        impl <
            V: NormalizedAction,
            $($keep_i: IntoIterator<Item = $keep_type>,)*
            $($($remove_i: IntoIterator<Item = $remove_type> + Clone,)*)*
            $($($remove_type: PartialEq + Eq,)*)*
            $($keep_type: $(ActionCmp<$remove_type> + )*,)*
            >
            TreeDedup
            <
            V,
            ($($keep_i,)*),
            ($($($remove_i,)*)*),
            ($($keep_type,)*),
            ($($($remove_type,)*)*)
            > for BlockTree<V>
            where
                $($keep_type: Into<V>,)*
                $($($remove_type: Into<V>,)*)*
            {
                #[allow(non_snake_case, unused_variables, unused_mut)]
                fn dedup_action_vec(mut keep_i: ($($keep_i,)*), mut remove_i: ($($($remove_i,)*)*))
                    -> Vec<V> {
                    let mut result = Vec::new();
                    // allow for each access to iters
                    let ($($keep_i,)*) = keep_i;
                    let ($($($remove_i,)*)*) = remove_i;

                    // for each keep iter, we check if it is a is_superior_action to any of
                    // the remove nodes, if this is true, we cache the remove node.
                    // once we have our set of bad nodes, we
                    let mut filtered: Vec<V> = Vec::new();
                    $(
                         $keep_i.into_iter().for_each(|keep| {
                             $(
                                 let cloned_iter = $remove_i.clone();
                                 for c_entry in cloned_iter.into_iter(){
                                    if keep.is_superior_action(&c_entry) {
                                         filtered.push(c_entry.into());
                                     }
                                  }
                               )*
                                result.push(keep.into());
                         });
                      )*

                    $(
                        $(
                            $remove_i.into_iter()
                                .map(Into::into)
                                .filter(|e| !filtered.contains(e))
                                .for_each(|e| result.push(e.into()));
                        )*
                     )*

                    result
                }
            }

    };
}

tree_dedup!();
tree_dedup!((KI0, [RI0, RT0], KT0));
tree_dedup!((KI0, [RI0, RT0], KT0), (KI1, [RI1, RT1], KT1));
tree_dedup!((KI0, [RI0, RT0], KT0), (KI1, [RI1, RT1], KT1), (KI2, [RI2, RT2], KT2));
tree_dedup!(
    (KI0, [RI0, RT0], KT0),
    (KI1, [RI1, RT1], KT1),
    (KI2, [RI2, RT2], KT2),
    (KI3, [RI3, RT3], KT3)
);
tree_dedup!(
    (KI0, [RI0, RT0], KT0),
    (KI1, [RI1, RT1], KT1),
    (KI2, [RI2, RT2], KT2),
    (KI3, [RI3, RT3], KT3),
    (KI4, [RI4, RT4], KT4)
);

pub trait InTupleFnOutVec<V: NormalizedAction> {
    type Out;
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
