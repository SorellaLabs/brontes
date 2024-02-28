use std::{iter::Iterator, sync::Arc};

use crate::{normalized_actions::NormalizedAction, BlockTree};

pub struct FilterMapTree<V: NormalizedAction, I, F> {
    pub tree: Arc<BlockTree<V>>,
    pub iter: I,
    pub f:    F,
}

impl<V: NormalizedAction, B, I: Iterator, F> Iterator for FilterMapTree<V, I, F>
where
    F: FnMut(Arc<BlockTree<V>>, I::Item) -> Option<B>,
{
    type Item = B;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(next) = self.iter.next() {
            if let Some(i) = (self.f)(self.tree.clone(), next) {
                return Some(i)
            }
        }

        None
    }
}
