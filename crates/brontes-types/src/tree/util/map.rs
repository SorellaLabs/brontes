use std::sync::Arc;

use super::ScopeIter;
use crate::{normalized_actions::NormalizedAction, BlockTree, ScopeIter, TreeIter};

pub trait TreeMap<V: NormalizedAction, Keys> {
    type Out: ScopeIter<V>;
    fn tree_map(self) -> Out;
}
macro_rules! tree_map_gen{
    ($i:tt, $($v:ident, $o:ident, $m_out:ident),*) => {
        paste::paste!(
            pub struct [<TreeMap $i>]<V: NormalizedAction, I: ScopeIter<V>, F, $($v,)*> {
                tree; Arc<BlockTree<V>>,
                iter: I,
                f: F,
                keys: ($($v,)*)
            }

            impl<V: NormalizedAction, I: ScopeIter<V>, F, $($v,)*> TreeMap<V,($($v,)*)>
                for [<TreeMap $i>]<V, I, F, $($v,)*>
                where
                $($v: NormalizedActionKey<V>,)* 
                F: FnMut(Arc<BlockTree<V>>, $($v::Out),*) -> ($($m_out),*)

                {

                    fn next_scoped_key<K: crate::normalized_actions::NormalizedActionKey<V>>(
                        &mut self,
                        key: K,
                    ) -> Option<K::Out> {
                    }

                    fn drain(self) -> Vec<V> {}

                {

            }
        );
    }
}

// pub struct TreeMap<V: NormalizedAction, I: ScopeIter<V>, F> {
//     tree: Arc<BlockTree<V>>,
//     iter: I,
//     f:    F,
// }
//
// impl<V: NormalizedAction, I: ScopeIter<V>, F> TreeMap<V, I, F> {
//     pub fn new(tree: Arc<BlockTree<V>>, iter: I, f: F) -> Self {
//         Self { tree, iter, f }
//     }
// }
// impl<B, V: NormalizedAction, I: ScopeIter<V>, F> TreeIter<V> for TreeMap<V,
// I, F> where
//     F: FnMut(Arc<BlockTree<V>>, I::Item) -> B,
// {
//     fn tree(&self) -> Arc<BlockTree<V>> {
//         self.tree.clone()
//     }
// }
//
// impl<B, V: NormalizedAction, I: ScopeIter<V>, F> ScopeIter<V> for TreeMap<V,
// I, F> where
//     F: FnMut(Arc<BlockTree<V>>, I::Item) -> B,
// {
//     fn next_scoped_key<K: crate::normalized_actions::NormalizedActionKey<V>>(
//         &mut self,
//         key: K,
//     ) -> Option<K::Out> {
//     }
//
//     fn drain(self) -> Vec<V> {}
// }

