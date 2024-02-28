use std::{marker::PhantomData, sync::Arc};

use crate::{normalized_actions::NormalizedAction, BlockTree, ScopeIter, TreeIter};

// pub trait FilterTree<V: NormalizedAction, Out, Keys, F> {
//     fn filter_tree(self, keys: Keys, f: F) -> Out;
// }
//
// macro_rules! tree_filter_gen {
//     ($i:tt, $($v:ident),*) => {
//         paste::paste!(
//             pub struct [<TreeFilter $i>]<V: NormalizedAction, I:
// ScopeIter<V>, F, $($v,)*> {                 tree: Arc<BlockTree<V>>,
//                 iter: I,
//                 f: F,
//                 keys: ($($v,)*),
//             }
//
//             #[allow(unused_parens)]
//             impl <V: NormalizedAction, I, F, $($v,)*>
//             FilterTree<
//             V,
//             [<TreeFilter $i>]<V, I, F, $($v,)*>,
//             ($($v,)*),
//             F
//             > for I where I: ScopeIter<V, Items = ($($v::Out,)*)> +
//             > TreeIter<V>, $($v: NormalizedActionKey<V>,)* F:
//             > FnMut(Arc<BlockTree<V>>, $(&Option<$v::Out>),*) -> bool
//             {
//                 fn filter_tree(self, keys: ($($v,)*), f: F) -> [<TreeFilter
// $i>]<V, I, F, $($v,)*> {                     [<TreeFilter $i>] {
//                         tree: self.tree(),
//                         iter: self,
//                         f,
//                         keys
//                     }
//
//                 }
//             }
//
//             #[allow(unused_parens)]
//             impl<V: NormalizedAction, I: ScopeIter<V>, F, $($v,)*>
// ScopeIter<V>                 for [<TreeFilter $i>]<V, I, F, $($v,)*>
//                 where
//
//                 I: ScopeIter<V, Items = ($($v::Out,)*)> + TreeIter<V>,
//                 $($v: NormalizedActionKey<V>,)*
//                 F: FnMut(Arc<BlockTree<V>>, $(&Option<$v::Out>),*) -> bool
//                 {
//                     type Acc = I::Acc;
//                     type Items = I::Items;
//
//                     fn next(&mut self) -> Option<Self::Items> {
//                         let ($($v,)*) = &self.keys;
//                         let ($($v,)*) = ($($v.clone(),)*);
//
//                         let mut all_good = true;
//                         let ($(mut [<key_ $v>],)*) = ($(None::<$v::Out>,)*);
//                         // collect all keys
//                         $(
//                             if let Some(inner) = self.next_scoped_key(&$v) {
//                                 [<key_ $v>] = Some(inner);
//                             } else {
//                                 all_good = false;
//                             }
//                         )*
//
//                         if all_good && (&mut self.f)(self.tree.clone(),
// $(&[<key_ $v>]),*)  {                             return Some(($([<key_
// $v>].unwrap(),)*))                         }
//
//                         None
//                     }
//
//                     fn next_scoped_key<K: NormalizedActionKey<V>>(
//                         &mut self,
//                         key: &K,
//                     ) -> Option<K::Out> {
//                         // check if this iter has the key. if it does,
//                         // then it means that it maps on it and there is no
// keys left                         let ($($v,)*) = &self.keys;
//                         $(
//                             if key.get_key().id == $v.get_key().id {
//                                 return None
//                             }
//                         )*
//
//                         self.iter.next_scoped_key(key)
//                     }
//
//                     fn drain(self) -> Vec<Self::Acc> {
//                         self.iter.drain()
//                     }
//             }
//         );
//     }
// }
//
// tree_filter_gen!(1, A);
// tree_filter_gen!(2, A, B);
// tree_filter_gen!(3, A, B, C);
// tree_filter_gen!(4, A, B, C, D);
// tree_filter_gen!(5, A, B, C, D, E);
//
// pub trait Filter<V: NormalizedAction, Out, Keys, F>
// where
//     Out: ScopeIter<V>,
// {
//     fn filter(self, keys: Keys, f: F) -> Out;
// }
//
// macro_rules! filter_gen {
//     ($i:tt, $($v:ident),*) => {
//         paste::paste!(
//             pub struct [<Filter $i>]<V: NormalizedAction, I: ScopeIter<V>, F,
// $($v,)*> {                 iter: I,
//                 f: F,
//                 keys: ($($v,)*),
//                 _p: PhantomData<V>
//             }
//
//             #[allow(unused_parens)]
//             impl <V: NormalizedAction, I, F, $($v,)*>
//             Filter<
//             V,
//             [<Filter $i>]<V, I, F, $($v,)*>,
//             ($($v,)*),
//             F
//             > for I where I: ScopeIter<V, Items = ($($v::Out,)*)> +
//             > TreeIter<V>, $($v: NormalizedActionKey<V>,)* F:
//             > FnMut($(&Option<$v::Out>),*) -> bool
//             {
//                 fn filter(self, keys: ($($v,)*), f: F) -> [<Filter $i>]<V, I,
// F, $($v,)*> {                     [<Filter $i>] {
//                         iter: self,
//                         f,
//                         keys,
//                         _p: PhantomData::default()
//                     }
//
//                 }
//             }
//             #[allow(unused_parens)]
//             impl<V: NormalizedAction, I: ScopeIter<V>, F, $($v,)*>
// ScopeIter<V>                 for [<Filter $i>]<V, I, F, $($v,)*>
//                 where
//
//                 I: ScopeIter<V, Items = ($($v::Out,)*)> + TreeIter<V>,
//                 $($v: NormalizedActionKey<V>,)*
//                 F: FnMut($(&Option<$v::Out>),*) -> bool
//                 {
//                     type Acc = I::Acc;
//                     type Items = I::Items;
//
//                     fn next(&mut self) -> Option<Self::Items> {
//                         let ($($v,)*) = &self.keys;
//                         let ($($v,)*) = ($($v.clone(),)*);
//
//                         let mut all_good = true;
//                         let ($(mut [<key_ $v>],)*) = ($(None::<$v::Out>,)*);
//                         // collect all keys
//                         $(
//                             if let Some(inner) = self.next_scoped_key(&$v) {
//                                 [<key_ $v>] = Some(inner);
//                             } else {
//                                 all_good = false;
//                             }
//                         )*
//
//                         if all_good && (&mut self.f)($(&[<key_ $v>]),*)  {
//                             return Some(($([<key_ $v>].unwrap(),)*))
//                         }
//
//                         None
//                     }
//
//                     fn next_scoped_key<K: NormalizedActionKey<V>>(
//                         &mut self,
//                         key: &K,
//                     ) -> Option<K::Out> {
//                         // check if this iter has the key. if it does,
//                         // then it means that it maps on it and there is no
// keys left                         let ($($v,)*) = &self.keys;
//                         $(
//                             if key.get_key().id == $v.get_key().id {
//                                 return None
//                             }
//                         )*
//
//                         self.iter.next_scoped_key(key)
//                     }
//
//                     fn drain(self) -> Vec<Self::Acc> {
//                         self.iter.drain()
//                     }
//             }
//         );
//     }
// }
//
// filter_gen!(1, A);
// filter_gen!(2, A, B);
// filter_gen!(3, A, B, C);
// filter_gen!(4, A, B, C, D);
// filter_gen!(5, A, B, C, D, E);
