use std::{collections::VecDeque, marker::PhantomData, sync::Arc};

use super::{ScopeIter, ScopeKey};
use crate::{normalized_actions::NormalizedAction, BlockTree, SplitIterZip, TreeIter};

pub trait TreeMap<V: NormalizedAction, Out, Keys, F> {
    fn tree_map(self, f: F) -> Out;
}

pub trait TreeMapAll<V: NormalizedAction, Out, Keys, F> {
    fn tree_map_all(self, f: F) -> Out;
}

macro_rules! tree_map_gen_all {
    ($i:tt, $b:ident, $($v:ident),*) => {
        paste::paste!(
            pub struct [<TreeMapAll $i>]<$b, I0, I1: Iterator, V: NormalizedAction, I: ScopeIter<I1>, F, $($v,)*> {
                tree: Arc<BlockTree<V>>,
                iter: I,
                f: F,
                buf: VecDeque<$b>,
                _p: PhantomData<(I0, I1,$($v,)*)>
            }

            impl <$b, I0, I1: Iterator,V: NormalizedAction, I: ScopeIter<I1>, F, $($v,)*> TreeIter<V>
                for [<TreeMapAll $i>]<$b, I0, I1,V, I, F, $($v,)*> {

                fn tree(&self) -> Arc<BlockTree<V>> {
                    self.tree.clone()
                }
            }

            #[allow(unused_parens)]
            impl <I0, I1: Iterator, V: NormalizedAction, I, F, $($v,)* $b >
            TreeMapAll<
            V,
            [<TreeMapAll $i>]<$b, I0, I1, V, I, F, $($v,)*>,
            ($($v),*),
            F,
            > for I
                where
                    I: ScopeIter<I1> + TreeIter<V>,
                    $($v: ScopeKey,)*
                    F: FnMut(Arc<BlockTree<V>>, $(Vec<$v>),*) -> Vec<$b>
            {
                fn tree_map_all(self, f: F) -> [<TreeMapAll $i>]<$b,
                    I0,
                    I1,
                    V, I, F, $($v),*> {
                    [<TreeMapAll $i>] {
                        tree: self.tree(),
                        iter: self,
                        buf: VecDeque::default(),
                        f,
                        _p: PhantomData::default()
                    }

                }
            }

            #[allow(unused_parens, non_snake_case)]
            impl<
                I0: Iterator + SplitIterZip<std::vec::IntoIter<$b>>,
                V: NormalizedAction,
                I,
                FN,
                $($v,)*
                $b
            > ScopeIter<<I0 as SplitIterZip<std::vec::IntoIter<$b>>>::Out>
                for [<TreeMapAll $i>]<$b, <I0 as SplitIterZip<std::vec::IntoIter<$b>>>::Out, I0, V, I, FN, $($v,)*>
                where
                I: ScopeIter<I0>,
                $($v: ScopeKey,)*
                FN: FnMut(Arc<BlockTree<V>>, $(Vec<$v>),*) -> Vec<$b>
                {
                    type Acc = I::Acc;
                    type Items = $b;

                    fn next(&mut self) -> Option<Self::Items> {
                        let mut any_none = false;
                        let ($(mut [<key_ $v>],)*) = ($(Vec::<$v>::new(),)*);
                        // collect all keys
                        $(
                            if let Some(inner) = self.iter.next_scoped_key::<$v>() {
                                [<key_ $v>].push(inner);
                            } else {
                                any_none =true;
                            }

                            while let Some(inner) = self.iter.next_scoped_key::<$v>() {
                                [<key_ $v>].push(inner);
                            }
                        )*

                        if any_none {
                            return None
                        }

                        let res = (&mut self.f)(self.tree.clone(), $([<key_ $v>]),*);
                        self.buf.extend(res);
                        self.buf.pop_front()
                    }

                    fn next_scoped_key<K: ScopeKey>(
                        &mut self,
                    ) -> Option<K> {
                        // check if this iter has the key. if it does,
                        // then it means that it maps on it and there is no keys left
                        $(
                            if K::ID == $v::ID {
                                return None
                            }
                        )*

                        self.iter.next_scoped_key()
                    }

                    fn drain(self) -> Vec<Self::Acc> {
                        self.iter.drain()
                    }

                    fn fold(mut self) -> <I0 as SplitIterZip<std::vec::IntoIter<$b>>>::Out {
                        let mut i = Vec::new();
                        while let Some(n) = self.next() {
                            i.push(n);
                        }
                        let b = self.iter.fold();
                        b.zip_with_inner(i.into_iter())
                    }
            }
        );
    }
}
tree_map_gen_all!(1, T0, T1);
tree_map_gen_all!(2, T0, T1, T2);
tree_map_gen_all!(3, T0, T1, T2, T3);
tree_map_gen_all!(4, T0, T1, T2, T3, T4);

macro_rules! tree_map_gen {
    ($i:tt, $b:ident, $($v:ident),*) => {
        paste::paste!(
            pub struct [<TreeMap $i>]<I0, I1: Iterator, V: NormalizedAction, I: ScopeIter<I1>, F, $($v,)*> {
                tree: Arc<BlockTree<V>>,
                iter: I,
                f: F,
                _p: PhantomData<(I0, I1,$($v,)*)>
            }

            impl <I0, I1: Iterator,V: NormalizedAction, I: ScopeIter<I1>, F, $($v,)*> TreeIter<V>
                for [<TreeMap $i>]<I0, I1,V, I, F, $($v,)*> {

                fn tree(&self) -> Arc<BlockTree<V>> {
                    self.tree.clone()
                }
            }

            #[allow(unused_parens)]
            impl <I0, I1: Iterator, V: NormalizedAction, I, F, $($v,)* $b >
            TreeMap<
            V,
            [<TreeMap $i>]<I0, I1, V, I, F, $($v,)*>,
            ($($v),*),
            F,
            > for I
                where
                    I: ScopeIter< I1> + TreeIter<V>,
                    $($v: ScopeKey,)*
                    F: FnMut(Arc<BlockTree<V>>, $($v),*) -> $b
            {
                fn tree_map(self, f: F) -> [<TreeMap $i>]<
                    I0,
                    I1,
                    V, I, F, $($v),*> {
                    [<TreeMap $i>] {
                        tree: self.tree(),
                        iter: self,
                        f,
                        _p: PhantomData::default()
                    }

                }
            }

            #[allow(unused_parens, non_snake_case)]
            impl<
                I0: Iterator + SplitIterZip<std::vec::IntoIter<$b>>,
                V: NormalizedAction,
                I,
                FN,
                $($v,)*
                $b
            > ScopeIter<<I0 as SplitIterZip<std::vec::IntoIter<$b>>>::Out>
                for [<TreeMap $i>]<<I0 as SplitIterZip<std::vec::IntoIter<$b>>>::Out, I0, V, I, FN, $($v,)*>
                where
                I: ScopeIter<I0>,
                $($v: ScopeKey,)*
                FN: FnMut(Arc<BlockTree<V>>, $($v),*) -> $b
                {
                    type Acc = I::Acc;
                    type Items = $b;

                    fn next(&mut self) -> Option<Self::Items> {

                        let mut any_none = false;
                        let ($(mut [<key_ $v>],)*) = ($(None::<$v>,)*);
                        // collect all keys
                        $(
                            if let Some(inner) = self.iter.next_scoped_key::<$v>() {
                                [<key_ $v>] = Some(inner);
                            } else {
                                any_none = true;
                            }
                        )*

                        if any_none {
                            return None
                        }

                        //
                        Some((&mut self.f)(self.tree.clone(), $([<key_ $v>].unwrap()),*))
                    }

                    fn next_scoped_key<K: ScopeKey>(
                        &mut self,
                    ) -> Option<K> {
                        // check if this iter has the key. if it does,
                        // then it means that it maps on it and there is no keys left
                        $(

                            if K::ID== $v::ID {
                                return None
                            }
                        )*

                        self.iter.next_scoped_key()
                    }

                    fn drain(self) -> Vec<Self::Acc> {
                        self.iter.drain()
                    }

                    fn fold(mut self) -> <I0 as SplitIterZip<std::vec::IntoIter<$b>>>::Out {
                        let mut i = Vec::new();
                        while let Some(n) = self.next() {
                            i.push(n);
                        }
                        let b = self.iter.fold();
                        b.zip_with_inner(i.into_iter())
                    }
            }
        );
    }
}
tree_map_gen!(1, T0, T1);
tree_map_gen!(2, T0, T1, T2);
tree_map_gen!(3, T0, T1, T2, T3);
tree_map_gen!(4, T0, T1, T2, T3, T4);

pub trait Map<V: NormalizedAction, Out, Keys, F> {
    fn map(self, f: F) -> Out;
}

macro_rules! map_gen {
    ($i:tt, $b:ident, $($v:ident),*) => {
        paste::paste!(
            pub struct [<Map $i>]<I0, I1: Iterator, V: NormalizedAction, I: ScopeIter<I1>, F, $($v,)*> {
                iter: I,
                f: F,
                _p: PhantomData<(V, I0, I1,$($v,)*)>
            }

            #[allow(unused_parens, non_snake_case)]
            impl <I0, I1: Iterator, V: NormalizedAction, I, F, $($v,)* $b >
            Map<
            V,
            [<Map $i>]<I0, I1, V, I, F, $($v,)*>,
            ($($v),*),
            F,
            > for I
                where
                    I: ScopeIter<I1>,
                    $($v: ScopeKey,)*
                    F: FnMut($($v),*) -> $b
            {
                fn map(self, f: F) -> [<Map $i>]<
                    I0,
                    I1,
                    V, I, F, $($v),*> {
                    [<Map $i>] {
                        iter: self,
                        f,
                        _p: PhantomData::default()
                    }

                }
            }

            #[allow(unused_parens, non_snake_case)]
            impl<
                I0: Iterator + SplitIterZip<std::vec::IntoIter<$b>>,
                I,
                V: NormalizedAction,
                FN,
                $($v,)*
                $b
            > ScopeIter<<I0 as SplitIterZip<std::vec::IntoIter<$b>>>::Out>
                for [<Map $i>]<<I0 as SplitIterZip<std::vec::IntoIter<$b>>>::Out, I0, V, I, FN, $($v,)*>
                where
                $($v: ScopeKey,)*
                FN: FnMut($($v),*) -> $b,
                I: ScopeIter<I0>,
                {
                    type Acc = I::Acc;
                    type Items = $b;

                    fn next(&mut self) -> Option<Self::Items> {
                        let mut any_none = false;
                        let ($(mut [<key_ $v>],)*) = ($(None::<$v>,)*);
                        // collect all keys
                        $(
                            if let Some(inner) = self.iter.next_scoped_key::<$v>() {
                                [<key_ $v>] = Some(inner);
                            } else  {
                                any_none = true;
                            }
                        )*

                        if any_none {
                            return None
                        }

                        //
                        Some((&mut self.f)($([<key_ $v>].unwrap()),*))
                    }

                    fn next_scoped_key<K: ScopeKey>(
                        &mut self,
                    ) -> Option<K> {
                        $(
                            if K::ID == $v::ID {
                                return None
                            }
                        )*

                        self.iter.next_scoped_key()
                    }

                    fn drain(self) -> Vec<Self::Acc> {
                        self.iter.drain()
                    }

                    fn fold(mut self) -> <I0 as SplitIterZip<std::vec::IntoIter<$b>>>::Out {
                        let mut i = Vec::new();
                        while let Some(n) = self.next() {
                            i.push(n);
                        }
                        let b = self.iter.fold();
                        b.zip_with_inner(i.into_iter())
                    }
            }
        );
    }
}

map_gen!(1, T0, T1);
map_gen!(2, T0, T1, T2);
map_gen!(3, T0, T1, T2, T3);
map_gen!(4, T0, T1, T2, T3, T4);
