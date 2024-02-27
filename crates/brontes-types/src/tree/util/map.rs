use std::{marker::PhantomData, sync::Arc};

use super::ScopeIter;
use crate::{
    normalized_actions::{NormalizedAction, NormalizedActionKey},
    BlockTree, TreeIter,
};

pub trait TreeMap<V: NormalizedAction, Out, Keys, F>: ScopeIter<V> + TreeIter<V>
where
    Out: ScopeIter<V>,
{
    fn tree_map(self, keys: Keys, f: F) -> Out;
}

macro_rules! tree_map_gen {
    ($i:tt, $(($v:ident, $m_out:ident)),*) => {
        paste::paste!(
            pub struct [<TreeMap $i>]<V: NormalizedAction, I: ScopeIter<V>, F, $($v,)*> {
                tree: Arc<BlockTree<V>>,
                iter: I,
                f: F,
                keys: ($($v,)*),
            }

            #[allow(unused_parens)]
            impl <V: NormalizedAction, I, F, $($v,)* $($m_out,)*>
            TreeMap<
            V,
            [<TreeMap $i>]<V, I, F, $($v,)*>,
            ($($v,)*),
            F
            > for I
                where
                    I: ScopeIter<V, Items = ($($v::Out,)*)> + TreeIter<V>,
                    $($v: NormalizedActionKey<V>,)*
                    F: FnMut(Arc<BlockTree<V>>, $(Option<$v::Out>),*) -> ($($m_out),*)
            {
                fn tree_map(self, keys: ($($v,)*), f: F) -> [<TreeMap $i>]<V, I, F, $($v,)*> {
                    [<TreeMap $i>] {
                        tree: self.tree(),
                        iter: self,
                        f,
                        keys
                    }

                }
            }

            #[allow(unused_parens)]
            impl<V: NormalizedAction, I: ScopeIter<V>, F, $($v,)* $($m_out,)*> ScopeIter<V>
                for [<TreeMap $i>]<V, I, F, $($v,)*>
                where
                $($v: NormalizedActionKey<V>,)*
                F: FnMut(Arc<BlockTree<V>>, $(Option<$v::Out>),*) -> ($($m_out),*)
                {
                    type Acc = V;
                    type Items = ($($m_out),*);
                    fn next(&mut self) -> Option<Self::Items> {
                        let ($($v,)*) = &self.keys;
                        let mut all_none = true;
                        let ($([<key_ $v>],)*) = ($(None::<$v>,)*);
                        // collect all keys
                        $(
                            if let Some(inner) = self.next_scoped_key($v) {
                                all_none = false;
                                [<key_ $v>] = Some(inner);
                            }
                        )*

                        if all_none {
                            return None
                        }

                        // run map fn
                        Some((&mut self.f)(self.tree.clone(), $([<key_ $v>]),*))
                    }

                    fn next_scoped_key<K: crate::normalized_actions::NormalizedActionKey<V>>(
                        &mut self,
                        key: &K,
                    ) -> Option<K::Out> {
                         self.iter.next_scoped_key(key)
                    }

                    fn drain(self) -> Vec<V> {
                        self.iter.drain()
                    }
            }
        );
    }
}

tree_map_gen!(1, (A, A0));
tree_map_gen!(2, (A, A0), (B, B0));
tree_map_gen!(3, (A, A0), (B, B0), (C, C0));
tree_map_gen!(4, (A, A0), (B, B0), (C, C0), (D, D0));
tree_map_gen!(5, (A, A0), (B, B0), (C, C0), (D, D0), (E, E0));

pub trait Map<V: NormalizedAction, Out, Keys, F>: ScopeIter<V>
where
    Out: ScopeIter<V>,
{
    fn map(self, keys: Keys, f: F) -> Out;
}

macro_rules! map_gen {
    ($i:tt, $(($v:ident, $m_out:ident)),*) => {
        paste::paste!(
            pub struct [<Map $i>]<V: NormalizedAction, I: ScopeIter<V>, F, $($v,)*> {
                iter: I,
                f: F,
                keys: ($($v,)*),
                _p: PhantomData<V>
            }

            #[allow(unused_parens)]
            impl <V: NormalizedAction, I, F, $($v,)* $($m_out,)*>
            Map<
            V,
            [<Map $i>]<V, I, F, $($v,)*>,
            ($($v,)*),
            F
            > for I
                where
                    I: ScopeIter<V, Items = ($($v::Out,)*)>,
                    $($v: NormalizedActionKey<V>,)*
                    F: FnMut($(Option<$v::Out>),*) -> ($($m_out),*)
            {
                fn map(self, keys: ($($v,)*), f: F) -> [<Map $i>]<V, I, F, $($v,)*> {
                    [<Map $i>] {
                        iter: self,
                        f,
                        keys,
                        _p: PhantomData::default()
                    }

                }
            }

            #[allow(unused_parens)]
            impl<V: NormalizedAction, I: ScopeIter<V>, F, $($v,)* $($m_out,)*> ScopeIter<V>
                for [<Map $i>]<V, I, F, $($v,)*>
                where
                $($v: NormalizedActionKey<V>,)*
                F: FnMut($(Option<$v::Out>),*) -> ($($m_out),*)
                {
                    type Acc = V;
                    type Items = ($($m_out),*);
                    fn next(&mut self) -> Option<Self::Items> {
                        let ($($v,)*) = &self.keys;
                        let mut all_none = true;
                        let ($([<key_ $v>],)*) = ($(None::<$v>,)*);
                        // collect all keys
                        $(
                            if let Some(inner) = self.next_scoped_key($v) {
                                all_none = false;
                                [<key_ $v>] = Some(inner);
                            }
                        )*

                        if all_none {
                            return None
                        }

                        // run map fn
                        Some((&mut self.f)($([<key_ $v>]),*))
                    }

                    fn next_scoped_key<K: crate::normalized_actions::NormalizedActionKey<V>>(
                        &mut self,
                        key: &K,
                    ) -> Option<K::Out> {
                         self.iter.next_scoped_key(key)
                    }

                    fn drain(self) -> Vec<V> {
                        self.iter.drain()
                    }
            }
        );
    }
}

map_gen!(1, (A, A0));
map_gen!(2, (A, A0), (B, B0));
map_gen!(3, (A, A0), (B, B0), (C, C0));
map_gen!(4, (A, A0), (B, B0), (C, C0), (D, D0));
map_gen!(5, (A, A0), (B, B0), (C, C0), (D, D0), (E, E0));
