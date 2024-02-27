use std::marker::PhantomData;

use crate::tree::NormalizedAction;

#[must_use = "iterators are lazy and do nothing unless consumed"]
pub struct FlattenSpecified<V: NormalizedAction, I: Iterator, W, T> {
    iter:      I,
    wanted:    W,
    transform: T,
    extra:     Vec<I::Item>,
    _p:        PhantomData<V>,
}

impl<V: NormalizedAction, I: Iterator, W, T> FlattenSpecified<V, I, W, T> {
    pub(crate) fn new(iter: I, wanted: W, transform: T) -> Self {
        Self { iter, wanted, transform, extra: vec![], _p: PhantomData::default() }
    }
}

impl<V: NormalizedAction, R: Clone, I: Iterator, W: Fn(&V) -> Option<&R>, T: Fn(R) -> Vec<V>>
    Iterator for FlattenSpecified<V, I, W, T>
{
    type Item = I::Item;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(extra) = self.extra.pop() {
            return Some(extra)
        }

        self.iter.next().and_then(|item| {
            if let Some(wanted) = (self.wanted)(&item) {
                let mut ret = (self.transform)(wanted.clone());
                let val = if ret.len() > 1 { Some(ret.remove(0)) } else { None };
                self.extra.extend(ret);
                val
            } else {
                Some(item)
            }
        })
    }
}
