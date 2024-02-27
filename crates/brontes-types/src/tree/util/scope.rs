use std::{collections::VecDeque, sync::Arc};

use super::TreeIter;
use crate::{normalized_actions::NormalizedActionKey, tree::NormalizedAction, BlockTree};

/// given a iterator of items that can be scoped out,
/// tracks the scoped items such that if in the future,
/// the scope changes, we can also pull from historical
pub trait ScopeIter<V: NormalizedAction> {
    type Items;
    type Acc;

    /// pulls the next item, this is used for default iter operations
    fn next(&mut self) -> Option<Self::Items>;
    /// fetches the next key value, allows us to scope based off of the key
    fn next_scoped_key<K: NormalizedActionKey<V>>(&mut self, key: &K) -> Option<K::Out>;
    /// all of the values that haven't been processed
    fn drain(self) -> Vec<Self::Acc>;
}

/// The base of the scoped iterator must be unified to work
pub struct ScopedIteratorBase<V: NormalizedAction, I: Iterator<Item = V>> {
    tree:          Arc<BlockTree<V>>,
    base_iterator: I,
    buf:           VecDeque<I::Item>,
}

impl<V: NormalizedAction, I: Iterator<Item = V>> ScopedIteratorBase<V, I> {
    pub fn new(tree: Arc<BlockTree<V>>, base_iterator: I) -> Self {
        Self { tree, base_iterator, buf: VecDeque::new() }
    }
}

impl<V: NormalizedAction, I: Iterator<Item = V>> TreeIter<V> for ScopedIteratorBase<V, I> {
    fn tree(&self) -> Arc<BlockTree<V>> {
        self.tree.clone()
    }
}

impl<V: NormalizedAction, I: Iterator<Item = V>> ScopeIter<V> for ScopedIteratorBase<V, I> {
    type Acc = V;
    type Items = V;

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
            if key.matches(val) {
                buf_i = Some(index);
                break
            }
        }

        if let Some(buf_index) = buf_i {
            return Some(key.into_val(self.buf.remove(buf_index)))
        }

        // if the buffer doesn't have the key then check the iterator
        while let Some(i) = self.base_iterator.next() {
            if key.matches(&i) {
                return Some(key.into_val(i))
            }
            // if doesn't match, add to buffer. this is so we can keep order while searching
            // through the whole list
            self.buf.push_back(i);
        }

        None
    }

    fn drain(self) -> Vec<Self::Acc> {
        self.buffer.into_iter().extend(self.base_iterator)
    }
}
