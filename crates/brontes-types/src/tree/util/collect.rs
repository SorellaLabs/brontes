use std::collections::HashMap;

use alloy_primitives::B256;

use crate::{normalized_actions::NormalizedAction, ActionIter, BlockTree, TreeSearchBuilder};

impl<V: NormalizedAction> TreeCollect<V> for BlockTree<V> {}

pub trait TreeCollect<V: NormalizedAction> {
    fn collect_all_action_filter<Ret>(
        &self,
        call: TreeSearchBuilder<V>,
        collector: fn(V) -> Option<Ret>,
    ) -> HashMap<B256, Vec<Ret>>
    where
        Self: TreeCollectCast<(Vec<Ret>,), (fn(V) -> Option<Ret>,), V>,
    {
        TreeCollectCast::collect_all_actions_filter(self, call, (collector,))
            .into_iter()
            .map(|(k, v)| (k, v.0))
            .collect()
    }

    fn collect_action_range_filter<Ret>(
        &self,
        range: Vec<B256>,
        call: TreeSearchBuilder<V>,
        collector: fn(V) -> Option<Ret>,
    ) -> HashMap<B256, Vec<Ret>>
    where
        Self: TreeCollectCast<(Vec<Ret>,), (fn(V) -> Option<Ret>,), V>,
    {
        TreeCollectCast::collect_actions_range_filter(self, range, call, (collector,))
            .into_iter()
            .map(|(k, v)| (k, v.0))
            .collect()
    }

    fn collect_action_filter<Ret>(
        &self,
        hash: B256,
        call: TreeSearchBuilder<V>,
        collector: fn(V) -> Option<Ret>,
    ) -> Vec<Ret>
    where
        Self: TreeCollectCast<(Vec<Ret>,), (fn(V) -> Option<Ret>,), V>,
    {
        TreeCollectCast::collect_actions_filter(self, hash, call, (collector,)).0
    }

    fn collect_all_actions_filter<FromI, Fns>(
        &self,
        call: TreeSearchBuilder<V>,
        collector: Fns,
    ) -> HashMap<B256, FromI>
    where
        Self: TreeCollectCast<FromI, Fns, V>,
    {
        TreeCollectCast::collect_all_actions_filter(self, call, collector)
    }

    fn collect_actions_range_filter<FromI, Fns>(
        &self,
        range: Vec<B256>,
        call: TreeSearchBuilder<V>,
        collector: Fns,
    ) -> HashMap<B256, FromI>
    where
        Self: TreeCollectCast<FromI, Fns, V>,
    {
        TreeCollectCast::collect_actions_range_filter(self, range, call, collector)
    }

    fn collect_actions_filter<FromI, Fns>(
        &self,
        hash: B256,
        call: TreeSearchBuilder<V>,
        collector: Fns,
    ) -> FromI
    where
        Self: TreeCollectCast<FromI, Fns, V>,
    {
        TreeCollectCast::collect_actions_filter(self, hash, call, collector)
    }
}

pub trait TreeCollectCast<FromI, Fns, V: NormalizedAction> {
    fn collect_all_actions_filter(
        &self,
        call: TreeSearchBuilder<V>,
        collector: Fns,
    ) -> HashMap<B256, FromI>;

    fn collect_actions_range_filter(
        &self,
        range: Vec<B256>,
        call: TreeSearchBuilder<V>,
        collector: Fns,
    ) -> HashMap<B256, FromI>;

    fn collect_actions_filter(
        &self,
        hash: B256,
        call: TreeSearchBuilder<V>,
        collector: Fns,
    ) -> FromI;
}

macro_rules! tree_cast {
    ($(($fns:ident, $ret:ident, $from:ident)),*) => {
        #[allow(non_snake_case)]
        impl<V: NormalizedAction, $($ret,)* $($fns: Fn(V) -> Option<$ret>),*,
        $($from: Default + Extend<$ret>),*>
            TreeCollectCast<($($from,)*), ($($fns,)*), V> for BlockTree<V> {
                fn collect_all_actions_filter(
                    &self,
                    call: TreeSearchBuilder<V>,
                    collector: ($($fns,)*),
                ) -> HashMap<B256, ($($from,)*)> {
                    self.collect_all(call).into_iter().map(|(k,v)| {
                        (k,
                         ActionIter::action_split_ref::<($($from,)*), ($($fns,)*)>
                            (v.into_iter(), &collector),
                         )
                    }).collect::<HashMap<_,_>>()
                }

                fn collect_actions_range_filter(
                    &self,
                    range: Vec<B256>,
                    call: TreeSearchBuilder<V>,
                    collector: ($($fns,)*),
                ) -> HashMap<B256, ($($from,)*)> {
                    self.collect_txes(range, call).into_iter().map(|(k,v)| {
                        (k,
                         ActionIter::action_split_ref::<($($from,)*), ($($fns,)*)>
                            (v.into_iter(), &collector),
                         )
                    }).collect::<HashMap<_,_>>()
                }

                fn collect_actions_filter(
                    &self,
                    hash: B256,
                    call: TreeSearchBuilder<V>,
                    collector: ($($fns,)*),
                ) -> ($($from,)*){
                    ActionIter::action_split::<($($from,)*), ($($fns,)*)>
                        (self.collect(hash,call).into_iter(), collector)
                }

        }
    };
}

tree_cast!((A, RETA, FA));
tree_cast!((A, RETA, FA), (B, RETB, FB));
tree_cast!((A, RETA, FA), (B, RETB, FB), (C, RETC, FC));
tree_cast!((A, RETA, FA), (B, RETB, FB), (C, RETC, FC), (D, RETD, FD));
tree_cast!((A, RETA, FA), (B, RETB, FB), (C, RETC, FC), (D, RETD, FD), (E, RETE, FE));
tree_cast!(
    (A, RETA, FA),
    (B, RETB, FB),
    (C, RETC, FC),
    (D, RETD, FD),
    (E, RETE, FE),
    (F, RETF, FF)
);
tree_cast!(
    (A, RETA, FA),
    (B, RETB, FB),
    (C, RETC, FC),
    (D, RETD, FD),
    (E, RETE, FE),
    (F, RETF, FF),
    (G, RETG, FG)
);
