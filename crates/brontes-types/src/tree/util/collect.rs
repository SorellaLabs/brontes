use alloy_primitives::B256;

use crate::{normalized_actions::NormalizedAction, ActionIter, BlockTree, TreeSearchBuilder};

impl<V: NormalizedAction> TreeCollect<V> for BlockTree<V> {}

pub trait TreeCollect<V: NormalizedAction> {
    fn collect_all_action_filter<'a, Ret>(
        &self,
        call: TreeSearchBuilder<V>,
        collector: fn(V) -> Option<Ret>,
    ) -> impl Iterator<Item = (B256, Vec<Ret>)> + 'a
    where
        Self: TreeCollectCast<(Vec<Ret>,), (fn(V) -> Option<Ret>,), V>,
        Ret: 'a,
        V: 'a,
    {
        TreeCollectCast::collect_all_actions_filter(self, call, (collector,)).map(|(k, v)| (k, v.0))
    }

    fn collect_action_range_filter<'a, Ret>(
        &'a self,
        range: &'a [B256],
        call: TreeSearchBuilder<V>,
        collector: fn(V) -> Option<Ret>,
    ) -> impl Iterator<Item = Vec<Ret>> + 'a
    where
        Self: TreeCollectCast<(Vec<Ret>,), (fn(V) -> Option<Ret>,), V>,
        Ret: 'a,
        V: 'a,
    {
        TreeCollectCast::collect_actions_range_filter(self, range, call, (collector,)).map(|v| v.0)
    }

    fn collect_action_filter<'a, Ret>(
        &self,
        hash: &B256,
        call: TreeSearchBuilder<V>,
        collector: fn(V) -> Option<Ret>,
    ) -> impl Iterator<Item = Ret> + 'a
    where
        Self: TreeCollectCast<(Vec<Ret>,), (fn(V) -> Option<Ret>,), V>,
        Ret: 'a,
        V: 'a,
    {
        TreeCollectCast::collect_actions_filter(self, hash, call, (collector,))
            .0
            .into_iter()
    }

    fn collect_all_actions_filter<'a, FromI, Fns>(
        &self,
        call: TreeSearchBuilder<V>,
        collector: Fns,
    ) -> impl Iterator<Item = (B256, FromI)> + 'a
    where
        Self: TreeCollectCast<FromI, Fns, V>,
        Fns: 'a,
        FromI: 'a,
        V: 'a,
    {
        TreeCollectCast::collect_all_actions_filter(self, call, collector)
    }

    fn collect_actions_range_filter<'a, FromI, Fns>(
        &'a self,
        range: &'a [B256],
        call: TreeSearchBuilder<V>,
        collector: Fns,
    ) -> impl Iterator<Item = FromI> + 'a
    where
        Self: TreeCollectCast<FromI, Fns, V>,
        Fns: 'a,
        FromI: 'a,
        V: 'a,
    {
        TreeCollectCast::collect_actions_range_filter(self, range, call, collector)
    }

    fn collect_actions_filter<'a, FromI, Fns>(
        &self,
        hash: &B256,
        call: TreeSearchBuilder<V>,
        collector: Fns,
    ) -> FromI
    where
        Self: TreeCollectCast<FromI, Fns, V>,
        Fns: 'a,
        FromI: 'a,
        V: 'a,
    {
        TreeCollectCast::collect_actions_filter(self, hash, call, collector)
    }
}

pub trait TreeCollectCast<FromI, Fns, V: NormalizedAction> {
    fn collect_all_actions_filter<'a>(
        &self,
        call: TreeSearchBuilder<V>,
        collector: Fns,
    ) -> impl Iterator<Item = (B256, FromI)> + 'a
    where
        Fns: 'a,
        FromI: 'a,
        V: 'a;

    fn collect_actions_range_filter<'a>(
        &'a self,
        range: &'a [B256],
        call: TreeSearchBuilder<V>,
        collector: Fns,
    ) -> impl Iterator<Item = FromI> + 'a
    where
        Fns: 'a,
        FromI: 'a,
        V: 'a;

    fn collect_actions_filter<'a>(
        &'a self,
        hash: &'a B256,
        call: TreeSearchBuilder<V>,
        collector: Fns,
    ) -> FromI
    where
        Fns: 'a,
        FromI: 'a,
        V: 'a;
}

macro_rules! tree_cast {
    ($(($fns:ident, $ret:ident, $from:ident)),*) => {
        #[allow(non_snake_case)]
        impl<V: NormalizedAction, $($ret,)* $($fns: Fn(V) -> Option<$ret>),*,
        $($from: Default + Extend<$ret>),*>
            TreeCollectCast<($($from,)*), ($($fns,)*), V> for BlockTree<V> {
                fn collect_all_actions_filter<'a>(
                    &self,
                    call: TreeSearchBuilder<V>,
                    collector: ($($fns,)*),
                ) -> impl Iterator<Item = (B256, ($($from,)*))> + 'a
                where
                    V: 'a,
                    $(
                        $fns: 'a,
                        $from: 'a,
                    )*
                {
                    self.collect_all(call).into_iter().map(move |(k,v)| {
                        (k,
                         ActionIter::action_split_ref::<($($from,)*), ($($fns,)*)>
                            (v.into_iter(), &collector),
                         )
                    })
                }

                fn collect_actions_range_filter<'a>(
                    &'a self,
                    range: &'a [B256],
                    call: TreeSearchBuilder<V>,
                    collector: ($($fns,)*),
                ) -> impl Iterator<Item = ($($from,)*)> +'a
                where
                    V: 'a,
                    $(
                        $fns: 'a,
                        $from: 'a,
                    )*
                {
                    self.collect_txes(range, call).into_iter().map(move |v| {
                         ActionIter::action_split_ref::<($($from,)*), ($($fns,)*)>
                            (v.into_iter(), &collector)
                    })
                }

                fn collect_actions_filter<'a>(
                    &self,
                    hash: &B256,
                    call: TreeSearchBuilder<V>,
                    collector: ($($fns,)*),
                ) -> ($($from,)*)
                where
                    V: 'a,
                    $(
                        $fns: 'a,
                        $from: 'a,
                    )*
                {
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
