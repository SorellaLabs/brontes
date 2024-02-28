pub mod collect;
pub mod core;
pub mod map;
pub use core::*;
use std::{collections::VecDeque, sync::Arc};

pub mod filter;
pub use filter::*;
pub mod change_scope;
pub use change_scope::*;
pub use collect::*;
pub use map::*;

use super::TreeIter;
use crate::{normalized_actions::NormalizedActionKey, tree::NormalizedAction, BlockTree};

/// given a iterator of items that can be scoped out,
/// tracks the scoped items such that if in the future,
/// the scope changes, we can also pull from historical
pub trait ScopeIter<V: NormalizedAction> {
    type Items;
    type Acc;
    type AccumRes;

    /// pulls the next item, this is used for default iter operations
    fn next(&mut self) -> Option<Self::Items>;
    /// fetches the next key value, allows us to scope based off of the key.
    fn next_scoped_key<K: NormalizedActionKey<V>>(&mut self, key: &K) -> Option<K::Out>;
    /// all of the values that haven't been processed
    fn drain(self) -> Vec<Self::Acc>;
    /// folds the iterator pulling all values
    fn fold<F: Default + Extend<Self::AccumRes>>(self) -> F;
}

/// The base of the scoped iterator must be unified to work
pub struct ScopedIteratorBase<V: NormalizedAction, I: Iterator> {
    tree:          Arc<BlockTree<V>>,
    base_iterator: I,
    buf:           VecDeque<I::Item>,
}

impl<V: NormalizedAction, I: Iterator> ScopedIteratorBase<V, I> {
    pub fn new(tree: Arc<BlockTree<V>>, base_iterator: I) -> Self {
        Self { tree, base_iterator, buf: VecDeque::new() }
    }
}

impl<V: NormalizedAction, I: Iterator> TreeIter<V> for ScopedIteratorBase<V, I> {
    fn tree(&self) -> Arc<BlockTree<V>> {
        self.tree.clone()
    }
}

impl<V: NormalizedAction, I: Iterator<Item = V>> ScopeIter<V> for ScopedIteratorBase<V, I> {
    type Acc = I::Item;
    type AccumRes = I::Item;
    type Items = I::Item;

    fn next(&mut self) -> Option<Self::Items> {
        if let Some(a) = self.buf.pop_front() {
            Some(a)
        } else {
            self.base_iterator.next()
        }
    }

    fn next_scoped_key<K: NormalizedActionKey<V>>(&mut self, key: &K) -> Option<K::Out> {
        // check the buffer for the next key
        let mut buf_i = None;
        for (index, val) in self.buf.iter().enumerate() {
            if (key.get_key().matches_ptr)(val) {
                buf_i = Some(index);
                break
            }
        }

        if let Some(buf_index) = buf_i {
            return (key.get_key().into_ptr)(self.buf.remove(buf_index).unwrap())
        }

        // if the buffer doesn't have the key then check the iterator
        while let Some(i) = self.base_iterator.next() {
            if (key.get_key().matches_ptr)(&i) {
                return (key.get_key().into_ptr)(i)
            }
            // if doesn't match, add to buffer. this is so we can keep order while searching
            // through the whole list
            self.buf.push_back(i);
        }

        None
    }

    fn drain(mut self) -> Vec<Self::Acc> {
        self.buf.extend(self.base_iterator);
        self.buf.into_iter().collect::<Vec<_>>()
    }
}

/// Tree wrapper for Scoped Iter
pub struct TreeIteratorScope<V: NormalizedAction, I: ScopeIter<V>> {
    tree: Arc<BlockTree<V>>,
    iter: I,
}

impl<I: ScopeIter<V>, V: NormalizedAction> TreeIteratorScope<V, I> {
    pub fn new(tree: Arc<BlockTree<V>>, iter: I) -> Self {
        Self { tree, iter }
    }
}

impl<I: ScopeIter<V>, V: NormalizedAction> TreeIter<V> for TreeIteratorScope<V, I> {
    fn tree(&self) -> Arc<BlockTree<V>> {
        self.tree.clone()
    }
}

impl<I: ScopeIter<V>, V: NormalizedAction> ScopeIter<V> for TreeIteratorScope<V, I> {
    type Acc = I::Acc;
    type Items = I::Items;

    fn next(&mut self) -> Option<Self::Items> {
        self.iter.next()
    }

    fn drain(self) -> Vec<Self::Acc> {
        self.iter.drain()
    }

    fn next_scoped_key<K: NormalizedActionKey<V>>(&mut self, key: &K) -> Option<K::Out> {
        self.iter.next_scoped_key(key)
    }
}
