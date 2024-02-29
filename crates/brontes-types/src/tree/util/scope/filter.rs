use std::{collections::VecDeque, marker::PhantomData, sync::Arc};

use super::{ScopeIter, ScopeKey};
use crate::{normalized_actions::NormalizedAction, BlockTree,  TreeIter};

pub trait TreeFilter<V: NormalizedAction, Out, Keys, F> {
    fn tree_filter(self, f: F) -> Out;
}

pub trait TreeFilterAll<V: NormalizedAction, Out, Keys, F> {
    fn tree_filter_all(self, f: F) -> Out;
}

macro_rules! tree_filter_gen_all {
    ($i:tt, $($v:ident),*) => {
        paste::paste!(
            pub struct [<TreeFilterAll $i>]<I1: Iterator, V: NormalizedAction, I: ScopeIter<I1>, F, $($v,)*> {
                tree: Arc<BlockTree<V>>,
                iter: I,
                f: F,
                buf: VecDeque<I::Items>,
                _p: PhantomData<( I1,$($v,)*)>
            }

            impl <I1: Iterator,V: NormalizedAction, I: ScopeIter<I1>, F, $($v,)*> TreeIter<V>
                for [<TreeFilterAll $i>]<I1,V, I, F, $($v,)*> {

                fn tree(&self) -> Arc<BlockTree<V>> {
                    self.tree.clone()
                }
            }

            #[allow(unused_parens)]
            impl <I1: Iterator, V: NormalizedAction, I, F, $($v,)* >
            TreeFilterAll<
            V,
            [<TreeFilterAll $i>]< I1, V, I, F, $($v,)*>,
            ($($v),*),
            F,
            > for I
                where
                    I: ScopeIter<I1> + TreeIter<V>,
                    $($v: ScopeKey,)*
                    F: FnMut(Arc<BlockTree<V>>, $(&[$v]),*) -> bool
            {
                fn tree_filter_all(self, f: F) -> [<TreeFilterAll $i>]<
                    I1,
                    V, I, F, $($v),*> {
                    [<TreeFilterAll $i>] {
                        tree: self.tree(),
                        iter: self,
                        f,
                        buf: VecDeque::default(),
                        _p: PhantomData::default()
                    }

                }
            }

            #[allow(unused_parens, non_snake_case)]
            impl<
                I0: Iterator,
                V: NormalizedAction,
                I,
                FN,
                $($v,)*
            > ScopeIter<I0>
                for [<TreeFilterAll $i>]<I0, V, I, FN, $($v,)*>
                where
                I: ScopeIter<I0,  Items = ($($v),*)> + Clone,
                $($v: ScopeKey,)*
                FN: FnMut(Arc<BlockTree<V>>, $(&[$v]),*) -> bool
                {
                    type Acc = I::Acc;
                    type Items = I::Items;

                    fn next(&mut self) -> Option<Self::Items> {
                        if let Some(next) = self.buf.pop_front() {
                            return Some(next)
                        }

                        let mut iter = self.iter.clone();
                        let ($(mut [<key_ $v>],)*) = ($(Vec::<$v>::new(),)*);
                        // collect all keys
                        while let Some(inner) = ScopeIter::next(&mut iter) {
                            let ($($v),*) = inner;
                            $(
                                [<key_ $v>].push($v);
                            )*
                        }
                        $(
                            if [<key_ $v>].is_empty() { return None }
                        )*

                        if (&mut self.f)(self.tree.clone(), $(&[<key_ $v>]),*) {
                            while let Some(next) = ScopeIter::next(&mut self.iter) {
                                self.buf.push_back(next);
                            }
                        }

                        self.buf.pop_front()
                    }

                    fn next_scoped_key<K: ScopeKey>(
                        &mut self,
                    ) -> Option<K> {
                        // check if this iter has the key. if it does,
                        // then we will pull from here, otherwise, we will go to a lower level to
                        // m
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

                    fn fold(self) -> I0 {
                        // self.iter
                        todo!()
                    }
            }
        );
    }
}
tree_filter_gen_all!(1, T0, T1);
tree_filter_gen_all!(2, T0, T1, T2);
tree_filter_gen_all!(3, T0, T1, T2, T3);
tree_filter_gen_all!(4, T0, T1, T2, T3, T4);

macro_rules! tree_filter_gen {
    ($i:tt, $($v:ident),*) => {
        paste::paste!(
            pub struct [<TreeFilter $i>]< I1: Iterator, V: NormalizedAction, I: ScopeIter<I1>, F, $($v,)*> {
                tree: Arc<BlockTree<V>>,
                iter: I,
                f: F,
                _p: PhantomData<(I1,$($v,)*)>
            }

            impl <I1: Iterator,V: NormalizedAction, I: ScopeIter<I1>, F, $($v,)*> TreeIter<V>
                for [<TreeFilter $i>]<I1,V, I, F, $($v,)*> {

                fn tree(&self) -> Arc<BlockTree<V>> {
                    self.tree.clone()
                }
            }

            #[allow(unused_parens)]
            impl <I1: Iterator, V: NormalizedAction, I, F, $($v,)* >
            TreeFilter<
            V,
            [<TreeFilter $i>]<I1, V, I, F, $($v,)*>,
            ($($v),*),
            F,
            > for I
                where
                    I: ScopeIter<I1> + TreeIter<V>,
                    $($v: ScopeKey,)*
                    F: FnMut(Arc<BlockTree<V>>, $($v),*) -> bool
            {
                fn tree_filter(self, f: F) -> [<TreeFilter $i>]<
                    I1,
                    V, I, F, $($v),*> {
                    [<TreeFilter $i>] {
                        tree: self.tree(),
                        iter: self,
                        f,
                        _p: PhantomData::default()
                    }
                }
            }

            #[allow(unused_parens, non_snake_case)]
            impl<
                I0: Iterator,
                V: NormalizedAction,
                I,
                FN,
                $($v,)*
            > ScopeIter<I>
                for [<TreeFilter $i>]<I0, V, I, FN, $($v,)*>
                where
                I: ScopeIter<I0> + Iterator,
                $($v: ScopeKey,)*
                FN: FnMut(Arc<BlockTree<V>>, $(&$v),*) -> bool
                {
                    type Acc = I::Acc;
                    type Items = I::Items;

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

                        if (&mut self.f)(self.tree.clone(), $([<key_ $v>].as_ref().unwrap()),*) {
                            return ScopeIter::next(&mut self.iter)
                        }

                        None
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

                    fn fold(self) -> I0 {
                        todo!()
                    }
            }
        );
    }
}
tree_filter_gen!(1, T1);
tree_filter_gen!(2, T1, T2);
tree_filter_gen!(3, T1, T2, T3);
tree_filter_gen!(4, T1, T2, T3, T4);
