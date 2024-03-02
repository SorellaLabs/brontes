pub mod collect;
pub mod core;
pub mod map;
pub use core::*;
pub mod scope_iter_base;
use std::{any::TypeId, marker::PhantomData, sync::Arc};

pub use scope_iter_base::*;

pub mod filter;
pub use filter::*;
pub mod change_scope;


pub use map::*;

use super::TreeIter;
use crate::{tree::NormalizedAction, BlockTree, SplitIterZip};

/// A key that allows for maping scoped data. this also allows for
/// a key with some grouped data
pub trait ScopeKey {
    const ID: TypeId;
}

impl<T: 'static> ScopeKey for T {
    const ID: TypeId = TypeId::of::<T>();
}

/// given a iterator of items that can be scoped out,
/// tracks the scoped items such that if in the future,
/// the scope changes, we can also opperate on these items
pub trait ScopeIter<IT>: Clone {
    type Items;
    type Acc;

    /// pulls the next item, this is used for default iter operations
    fn next(&mut self) -> Option<Self::Items>;
    /// fetches the next key value, allows us to scope based off of the key.
    fn next_scoped_key<K: ScopeKey>(&mut self) -> Option<K>;
    /// all of the values that haven't been processed
    fn drain(self) -> Vec<Self::Acc>;
    /// folds the iterator pulling all values
    fn fold(self) -> IT;
}

/// Tree wrapper for Scoped Iter
#[derive(Clone)]
pub struct TreeIteratorScope<K, U: Iterator + Clone, V: NormalizedAction, I: ScopeIter<U>> {
    tree: Arc<BlockTree<V>>,
    iter: I,
    _p:   PhantomData<(U, K)>,
}

impl<U: Iterator + Clone, I: ScopeIter<U>, V: NormalizedAction, K> TreeIteratorScope<K, U, V, I> {
    pub fn new(tree: Arc<BlockTree<V>>, iter: I) -> Self {
        Self { tree, iter, _p: PhantomData }
    }
}

impl<K, U: Iterator + Clone, I: ScopeIter<U>, V: NormalizedAction> TreeIter<V>
    for TreeIteratorScope<K, U, V, I>
{
    fn tree(&self) -> Arc<BlockTree<V>> {
        self.tree.clone()
    }
}

impl<
        IT: Iterator + SplitIterZip<std::vec::IntoIter<I::Items>> + Clone,
        I: ScopeIter<IT>,
        V: NormalizedAction,
    > ScopeIter<<IT as SplitIterZip<std::vec::IntoIter<I::Items>>>::Out>
    for TreeIteratorScope<<IT as SplitIterZip<std::vec::IntoIter<I::Items>>>::Out, IT, V, I>
where
    <IT as SplitIterZip<std::vec::IntoIter<I::Items>>>::Out: Clone,
{
    type Acc = I::Acc;
    type Items = I::Items;

    fn next(&mut self) -> Option<Self::Items> {
        self.iter.next()
    }

    fn drain(self) -> Vec<Self::Acc> {
        self.iter.drain()
    }

    fn next_scoped_key<K: ScopeKey>(&mut self) -> Option<K> {
        self.iter.next_scoped_key()
    }

    fn fold(mut self) -> <IT as SplitIterZip<std::vec::IntoIter<I::Items>>>::Out {
        let mut i = Vec::new();
        while let Some(n) = self.next() {
            i.push(n);
        }
        let b = self.iter.fold();
        b.zip_with_inner(i.into_iter())
    }
}
