use std::sync::Arc;

use crate::{normalized_actions::NormalizedAction, BlockTree};

pub struct FilterTree<V: NormalizedAction, I, F> {
    pub tree: Arc<BlockTree<V>>,
    pub iter: I,
    pub f: F,
}

impl<V: NormalizedAction, I: Iterator, F> Iterator for FilterTree<V, I, F>
where
    F: FnMut(Arc<BlockTree<V>>, &I::Item) -> bool,
{
    type Item = I::Item;

    fn next(&mut self) -> Option<Self::Item> {
        for next in self.iter.by_ref() {
            if (self.f)(self.tree.clone(), &next) {
                return Some(next);
            }
        }

        None
    }
}
