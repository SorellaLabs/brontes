use std::{marker::PhantomData, sync::Arc};

use super::ScopeIter;
use crate::{
    normalized_actions::{NormalizedAction, NormalizedActionKey},
    BlockTree, TreeIter,
};

pub trait TreeMap<V: NormalizedAction, Out, Keys, F>
where
    Out: ScopeIter<V>,
{
    fn tree_map(self, keys: Keys, f: F) -> Out;
}

macro_rules! tree_map_gen {
    ($i:tt, $b:ident, $($v:ident),*) => {
        paste::paste!(
            pub struct [<TreeMap $i>]<V: NormalizedAction, I: ScopeIter<V>, F, $($v,)*> {
                tree: Arc<BlockTree<V>>,
                iter: I,
                f: F,
                keys: ($($v),*),
            }

            impl <V: NormalizedAction, I: ScopeIter<V>, F, $($v,)*> TreeIter<V>
                for [<TreeMap $i>]<V, I, F, $($v,)*> {

                fn tree(&self) -> Arc<BlockTree<V>> {
                    self.tree.clone()
                }
            }



            #[allow(unused_parens)]
            impl <V: NormalizedAction, I, F, $($v,)* $b>
            TreeMap<
            V,
            [<TreeMap $i>]<V, I, F, $($v,)*>,
            ($($v),*),
            F
            > for I
                where
                    I: ScopeIter<V> + TreeIter<V>,
                    $($v: NormalizedActionKey<V>,)*
                    F: FnMut(Arc<BlockTree<V>>, $($v::Out),*) -> $b
            {
                fn tree_map(self, keys: ($($v),*), f: F) -> [<TreeMap $i>]<V, I, F, $($v,)*> {
                    [<TreeMap $i>] {
                        tree: self.tree(),
                        iter: self,
                        f,
                        keys,
                    }

                }
            }

            #[allow(unused_parens)]
            impl<V: NormalizedAction, I: ScopeIter<V>, F, $($v,)* $b> ScopeIter<V>
                for [<TreeMap $i>]<V, I, F, $($v,)*>
                where
                $($v: NormalizedActionKey<V>,)*
                F: FnMut(Arc<BlockTree<V>>, $($v::Out),*) -> $b
                {
                    type Acc = I::Acc;
                    type Items = $b;

                    fn next(&mut self) -> Option<Self::Items> {
                        let ($($v),*) = &self.keys;
                        let ($($v),*) = ($($v.clone()),*);

                        let mut any_none = false;
                        let ($(mut [<key_ $v>],)*) = ($(None::<$v::Out>,)*);
                        // collect all keys
                        $(
                            if let Some(inner) = self.iter.next_scoped_key(&$v) {
                                any_none = true;
                                [<key_ $v>] = Some(inner);
                            }
                        )*

                        if any_none {
                            return None
                        }

                        //
                        Some((&mut self.f)(self.tree.clone(), $([<key_ $v>].unwrap()),*))
                    }

                    fn next_scoped_key<K: NormalizedActionKey<V>>(
                        &mut self,
                        key: &K,
                    ) -> Option<K::Out> {
                        // check if this iter has the key. if it does,
                        // then it means that it maps on it and there is no keys left
                        let ($($v),*) = &self.keys;
                        $(
                            if key.get_key().id == $v.get_key().id {
                                return None
                            }
                        )*

                        self.iter.next_scoped_key(key)
                    }

                    fn drain(self) -> Vec<Self::Acc> {
                        self.iter.drain()
                    }
            }
        );
    }
}

tree_map_gen!(1, A, B);
tree_map_gen!(2, A, B, C);
tree_map_gen!(4, A, B, C, D);
tree_map_gen!(5, A, B, C, D, E);
tree_map_gen!(6, A, B, C, D, E, G);

pub trait Map<V: NormalizedAction, Out, Keys, F>
where
    Out: ScopeIter<V>,
{
    fn map(self, keys: Keys, f: F) -> Out;
}

macro_rules! map_gen {
    ($i:tt, $b:ident, $($v:ident),*) => {
        paste::paste!(
            pub struct [<Map $i>]<V: NormalizedAction, I: ScopeIter<V>, F, $($v,)*> {
                iter: I,
                f: F,
                keys: ($($v),*),
                _p: PhantomData<V>
            }

            #[allow(unused_parens)]
            impl <V: NormalizedAction, I, F, $($v,)* $b>
            Map<
            V,
            [<Map $i>]<V, I, F, $($v,)*>,
            ($($v),*),
            F
            > for I
                where
                    I: ScopeIter<V, Items = ($($v::Out,)*)>,
                    $($v: NormalizedActionKey<V>,)*
                    F: FnMut($(Option<$v::Out>),*) -> $b
            {
                fn map(self, keys: ($($v),*), f: F) -> [<Map $i>]<V, I, F, $($v,)*> {
                    [<Map $i>] {
                        iter: self,
                        f,
                        keys,
                        _p: PhantomData::default()
                    }

                }
            }

            #[allow(unused_parens)]
            impl<V: NormalizedAction, I: ScopeIter<V>, F, $($v,)* $b> ScopeIter<V>
                for [<Map $i>]<V, I, F, $($v,)*>
                where
                $($v: NormalizedActionKey<V>,)*
                F: FnMut($(Option<$v::Out>),*) -> $b,
                {
                    type Acc = I::Acc;
                    type Items = $b;

                    fn next(&mut self) -> Option<Self::Items> {
                        let ($($v),*) = &self.keys;
                        let ($($v),*) = ($($v.clone()),*);

                        let mut all_none = true;
                        let ($(mut [<key_ $v:lower>],)*) = ($(None::<$v::Out>,)*);
                        // collect all keys
                        $(
                            if let Some(inner) = self.next_scoped_key(&$v) {
                                all_none = false;
                                [<key_ $v:lower>] = Some(inner);
                            }
                        )*

                        if all_none {
                            return None
                        }

                        // run map fn
                        Some((&mut self.f)($([<key_ $v:lower>]),*))
                    }

                    fn next_scoped_key<K: crate::normalized_actions::NormalizedActionKey<V>>(
                        &mut self,
                        key: &K,
                    ) -> Option<K::Out> {
                        // check if this iter has the key. if it does,
                        // then it means that it maps on it and there is no keys left
                        let ($($v),*) = &self.keys;
                        $(
                            if key.get_key().id == $v.get_key().id {
                                return None
                            }
                        )*

                        self.iter.next_scoped_key(key)
                    }

                    fn drain(self) -> Vec<Self::Acc> {
                        self.iter.drain()
                    }
            }
        );
    }
}

map_gen!(1, A, B);
map_gen!(2, A, B, C);
map_gen!(4, A, B, C, D);
map_gen!(5, A, B, C, D, E);
map_gen!(6, A, B, C, D, E, G);
