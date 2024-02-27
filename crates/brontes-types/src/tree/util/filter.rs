use std::{marker::PhantomData, sync::Arc};

use crate::{
    normalized_actions::{NormalizedAction, NormalizedActionKey},
    BlockTree, ScopeIter, TreeIter,
};

pub trait FilterTree<V: NormalizedAction, Out, Keys, F>: ScopeIter<V> + TreeIter<V>
where
    Out: ScopeIter<V>,
{
    fn filter_tree(self, keys: Keys, f: F) -> Out;
}

macro_rules! tree_filter_gen {
    ($i:tt, $($v:ident),*) => {
        paste::paste!(
            pub struct [<TreeFilter $i>]<V: NormalizedAction, I: ScopeIter<V>, F, $($v,)*> {
                tree: Arc<BlockTree<V>>,
                iter: I,
                f: F,
                keys: ($($v,)*),
            }

            #[allow(unused_parens)]
            impl <V: NormalizedAction, I, F, $($v,)*>
            FilterTree<
            V,
            [<TreeFilter $i>]<V, I, F, $($v,)*>,
            ($($v,)*),
            F
            > for I
                where
                    I: ScopeIter<V, Items = ($($v::Out,)*)> + TreeIter<V>,
                    $($v: NormalizedActionKey<V>,)*
                    F: FnMut(Arc<BlockTree<V>>, $(&Option<$v::Out>),*) -> bool
            {
                fn filter_tree(self, keys: ($($v,)*), f: F) -> [<TreeFilter $i>]<V, I, F, $($v,)*> {
                    [<TreeFilter $i>] {
                        tree: self.tree(),
                        iter: self,
                        f,
                        keys
                    }

                }
            }

            #[allow(unused_parens)]
            impl<V: NormalizedAction, I: ScopeIter<V>, F, $($v,)*> ScopeIter<V>
                for [<TreeFilter $i>]<V, I, F, $($v,)*>
                where
                $($v: NormalizedActionKey<V>,)*
                F: FnMut(Arc<BlockTree<V>>, $(&Option<$v::Out>),*) -> bool
                {
                    type Acc = V;
                    type Items = ($($v::Out,)*);

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

                        if !all_none && (&mut self.f)(self.tree.clone(), $(&[<key_ $v>]),*)  {
                            return Some(($([<key_ $v>]),*))
                        }

                        None
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

tree_filter_gen!(1, A);
tree_filter_gen!(2, A, B);
tree_filter_gen!(3, A, B, C);
tree_filter_gen!(4, A, B, C, D);
tree_filter_gen!(5, A, B, C, D, E);

pub trait Filter<V: NormalizedAction, Out, Keys, F>: ScopeIter<V> + TreeIter<V>
where
    Out: ScopeIter<V>,
{
    fn filter(self, keys: Keys, f: F) -> Out;
}

macro_rules! filter_gen {
    ($i:tt, $($v:ident),*) => {
        paste::paste!(
            pub struct [<Filter $i>]<V: NormalizedAction, I: ScopeIter<V>, F, $($v,)*> {
                iter: I,
                f: F,
                keys: ($($v,)*),
                _p: PhantomData<V>
            }

            #[allow(unused_parens)]
            impl <V: NormalizedAction, I, F, $($v,)*>
            Filter<
            V,
            [<Filter $i>]<V, I, F, $($v,)*>,
            ($($v,)*),
            F
            > for I
                where
                    I: ScopeIter<V, Items = ($($v::Out,)*)> + TreeIter<V>,
                    $($v: NormalizedActionKey<V>,)*
                    F: FnMut($(&Option<$v::Out>),*) -> bool
            {
                fn filter(self, keys: ($($v,)*), f: F) -> [<Filter $i>]<V, I, F, $($v,)*> {
                    [<Filter $i>] {
                        iter: self,
                        f,
                        keys,
                        _p: PhantomData::default()
                    }

                }
            }

            #[allow(unused_parens)]
            impl<V: NormalizedAction, I: ScopeIter<V>, F, $($v,)*> ScopeIter<V>
                for [<Filter $i>]<V, I, F, $($v,)*>
                where
                $($v: NormalizedActionKey<V>,)*
                F: FnMut($(&Option<$v::Out>),*) -> bool
                {
                    type Acc = V;
                    type Items = ($($v::Out,)*);

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

                        if !all_none && (&mut self.f)($(&[<key_ $v>]),*)  {
                            return Some(($([<key_ $v>]),*))
                        }

                        None
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

filter_gen!(1, A);
filter_gen!(2, A, B);
filter_gen!(3, A, B, C);
filter_gen!(4, A, B, C, D);
filter_gen!(5, A, B, C, D, E);
