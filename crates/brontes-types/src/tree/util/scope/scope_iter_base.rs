use std::{collections::VecDeque, sync::Arc};

use super::ScopeKey;
use crate::{normalized_actions::NormalizedAction, BlockTree, ScopeIter, TreeIter};

pub trait ScopeIterBase<V: NormalizedAction, Out>: TreeIter<V> {
    fn scope_iter_base(self) -> Out;
}

macro_rules! scope_iter_base {
    ($i:tt, $($v:ident),*) => {
        paste::paste!(
            impl<IT:Clone, V: NormalizedAction, $($v:Clone,)*> ScopeIterBase<V,
            [<ScopeBase $i>]<V, IT, $($v,)*>> for IT
            where
                IT: TreeIter<V>,
                IT: Iterator<Item = ($($v),*)> {
                     fn scope_iter_base(self) -> [<ScopeBase $i>]<V, IT, $($v,)*> {
                        [<ScopeBase $i>] {
                            tree: self.tree(),
                            iter: self,
                            buf: VecDeque::default()
                        }
                     }
                }

            #[derive(Clone)]
            pub struct [<ScopeBase $i>]<V: NormalizedAction, I: Iterator + Clone, $($v: Clone,)*> {
                tree: Arc<BlockTree<V>>,
                iter: I,
                buf: VecDeque<($(Option<$v>),*)>,
            }

            impl < V: NormalizedAction, I: Iterator + Clone, $($v: Clone,)*> TreeIter<V>
                for [<ScopeBase $i>]< V, I, $($v,)*> {

                fn tree(&self) -> Arc<BlockTree<V>> {
                    self.tree.clone()
                }
            }

            #[allow(unused_parens, non_snake_case)]
            impl<
                V: NormalizedAction,
                I,
                $($v,)*
            > ScopeIter<I>
                for [<ScopeBase $i>]<V, I, $($v,)*>
                where
                $($v: ScopeKey + Clone,)*
                I: Iterator<Item = ($($v),*)> + Clone
                {
                    type Acc = I::Item;
                    type Items = ($($v),*);

                    fn next(&mut self) -> Option<Self::Items> {
                        self.iter.next()
                    }

                    fn next_scoped_key<K: ScopeKey>(
                        &mut self,
                    ) -> Option<K> {

                        if let Some(($($v),*)) = self.buf.pop_front().map(|i|Some(i))
                            .unwrap_or_else(||self.next().map(|($($v),*)| ($(Some($v)),*))) {
                            let mut inserts = ($(None::<$v>),*);
                            let ($([<$v _k>]),*) = &mut inserts;

                            $(
                                if let Some(value) = $v {
                                    if K::ID == $v::ID {
                                        self.buf.push_back(inserts);
                                        // if we have equal type ids, we have a key match,
                                        // and can just convert this value to the underlying
                                        return Some(unsafe {std::mem::transmute_copy(&value) })
                                    }
                                    *[<$v _k>] = Some(value);
                                }
                            )*
                        }

                        None
                    }

                    fn drain(self) -> Vec<Self::Acc> {
                        self.iter.collect::<Vec<_>>()
                    }

                    fn fold(self) -> I {
                        todo!("for when we forget");
                    }
            }
        );
    }
}

scope_iter_base!(1, T0);
scope_iter_base!(2, T0, T1);
scope_iter_base!(3, T0, T1, T2);
scope_iter_base!(4, T0, T1, T2, T3);
scope_iter_base!(5, T0, T1, T2, T3, T4);
