use alloy_primitives::B256;

use super::TreeSearchBuilder;
use crate::{
    normalized_actions::{Action, NormalizedAction},
    ActionIter, BlockTree, FastHashMap,
};

pub trait TreeUtils<V: NormalizedAction> {
    fn collect_all_action_filter<Ret>(
        &self,
        call: TreeSearchBuilder<V>,
        collector: fn(Action) -> Option<Ret>,
    ) -> FastHashMap<B256, Ret>
    where
        Self: TreeUtilsCast<(Ret,), (fn(Action) -> Option<Ret>,), V>,
    {
        TreeUtilsCast::collect_all_actions_filter(self, call, (collector,))
            .into_iter()
            .map(|(k, v)| (k, v.0))
            .collect()
    }

    fn collect_action_range_filter<Ret>(
        &self,
        range: Vec<B256>,
        call: TreeSearchBuilder<V>,
        collector: fn(Action) -> Option<Ret>,
    ) -> FastHashMap<B256, Ret>
    where
        Self: TreeUtilsCast<(Ret,), (fn(Action) -> Option<Ret>,), V>,
    {
        TreeUtilsCast::collect_actions_range_filter(self, range, call, (collector,))
            .into_iter()
            .map(|(k, v)| (k, v.0))
            .collect()
    }

    fn collect_action_filter<Ret>(
        &self,
        hash: B256,
        call: TreeSearchBuilder<V>,
        collector: fn(Action) -> Option<Ret>,
    ) -> Ret
    where
        Self: TreeUtilsCast<(Ret,), (fn(Action) -> Option<Ret>,), V>,
    {
        TreeUtilsCast::collect_actions_filter(self, hash, call, (collector,)).0
    }

    fn collect_all_actions_filter<FromI, Fns>(
        &self,
        call: TreeSearchBuilder<V>,
        collector: Fns,
    ) -> FastHashMap<B256, FromI>
    where
        Self: TreeUtilsCast<FromI, Fns, V>,
    {
        TreeUtilsCast::collect_all_actions_filter(self, call, collector)
    }

    fn collect_actions_range_filter<FromI, Fns>(
        &self,
        range: Vec<B256>,
        call: TreeSearchBuilder<V>,
        collector: Fns,
    ) -> FastHashMap<B256, FromI>
    where
        Self: TreeUtilsCast<FromI, Fns, V>,
    {
        TreeUtilsCast::collect_actions_range_filter(self, range, call, collector)
    }

    fn collect_actions_filter<FromI, Fns>(
        &self,
        hash: B256,
        call: TreeSearchBuilder<V>,
        collector: Fns,
    ) -> FromI
    where
        Self: TreeUtilsCast<FromI, Fns, V>,
    {
        TreeUtilsCast::collect_actions_filter(self, hash, call, collector)
    }
}

pub trait TreeUtilsCast<FromI, Fns, V: NormalizedAction> {
    fn collect_all_actions_filter(
        &self,
        call: TreeSearchBuilder<V>,
        collector: Fns,
    ) -> FastHashMap<B256, FromI>;

    fn collect_actions_range_filter(
        &self,
        range: Vec<B256>,
        call: TreeSearchBuilder<V>,
        collector: Fns,
    ) -> FastHashMap<B256, FromI>;

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
        impl<$($ret,)* $($fns: Fn(Action) -> Option<$ret>),*, $($from: Default + Extend<$ret>),*>
            TreeUtilsCast<($($from,)*), ($($fns,)*), Actions> for BlockTree<Action> {
                fn collect_all_actions_filter(
                    &self,
                    call: TreeSearchBuilder<Action>,
                    collector: ($($fns,)*),
                ) -> FastHashMap<B256, ($($from,)*)> {
                    self.collect_all(call).into_iter().map(|(k,v)| {
                        (k,
                         ActionIter::action_split_ref::<($($from,)*), ($($fns,)*)>
                            (v.into_iter(), &collector),
                         )
                    }).collect::<FastHashMap<_,_>>()
                }

                fn collect_actions_range_filter(
                    &self,
                    range: Vec<B256>,
                    call: TreeSearchBuilder<Action>,
                    collector: ($($fns,)*),
                ) -> FastHashMap<B256, ($($from,)*)> {
                    self.collect_txes(range, call).into_iter().map(|(k,v)| {
                        (k,
                         ActionIter::action_split_ref::<($($from,)*), ($($fns,)*)>
                            (v.into_iter(), &collector),
                         )
                    }).collect::<FastHashMap<_,_>>()
                }

                fn collect_actions_filter(
                    &self,
                    hash: B256,
                    call: TreeSearchBuilder<Action>,
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
