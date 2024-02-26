use std::marker::PhantomData;

fn test_beast(items: Vec<(u8, String)>) {
    let a = items
        .into_iter()
        .map_multisplit()
        .map_item_1(|a| a as u32)
        .map_item_2(|b| b)
        .map(|b| b);
}

pub trait IntoSplitIterator {
    type Item;
    type Iter: Iterator<Item = Self::Item>;

    fn into_split_iter(self) -> Self::Iter;
}

pub struct IntoSplitIterTwo<A, B> {
    iters: (A, B),
}

impl<A, B> IntoSplitIterator for IntoSplitIterTwo<A, B>
where
    A: IntoIterator,
    B: IntoIterator,
{
    type Item = (Option<A::Item>, Option<B::Item>);
    type Iter = ZipPadded<A::IntoIter, B::IntoIter>;

    fn into_split_iter(self) -> Self::Iter {
        let (a, b) = self.iters;
        a.into_iter().zip_padded(b)
    }
}

pub trait SplitIter<Item, K>: Iterator<Item = Item> {
    fn map_multisplit(self) -> K;
}

impl<I, A, B, F1, F2> SplitIter<(A, B), SplitIterTwo<A, B, I, false, false, F1, F2>> for I
where
    I: Iterator<Item = (A, B)>,
{
    fn map_multisplit(self) -> SplitIterTwo<A, B, I, false, false, F1, F2> {
        SplitIterTwo::<A, B, I, false, false, F1, F2>::new(self)
    }
}

pub struct SplitIterTwo<C, D, I: Iterator<Item = (C, D)>, const A: bool, const B: bool, F1, F2> {
    iter: I,
    fn1:  Option<F1>,
    fn2:  Option<F2>,
    _p:   PhantomData<(C, D)>,
}

impl<C, D, I: Iterator<Item = (C, D)>, const A: bool, const B: bool, F1, F2>
    SplitIterTwo<C, D, I, A, B, F1, F2>
{
    pub fn new(iter: I) -> SplitIterTwo<C, D, I, false, false, F1, F2> {
        SplitIterTwo { iter, fn1: None, fn2: None, _p: PhantomData::default() }
    }

    pub fn map_item_1<O>(mut self, fn1: F1) -> SplitIterTwo<C, D, I, true, B, F1, F2>
    where
        F1: Fn(C) -> O,
    {
        self.fn1 = Some(fn1);
        SplitIterTwo { fn1: self.fn1, iter: self.iter, fn2: self.fn2, _p: self._p }
    }

    pub fn map_item_2<O>(mut self, fn2: F2) -> SplitIterTwo<C, D, I, A, true, F1, F2>
    where
        F2: Fn(D) -> O,
    {
        self.fn2 = Some(fn2);
        SplitIterTwo { fn1: self.fn1, iter: self.iter, fn2: self.fn2, _p: self._p }
    }
}

impl<C, D, O1, O2, I: Iterator<Item = (C, D)>, F1, F2> Iterator
    for SplitIterTwo<C, D, I, true, true, F1, F2>
where
    F1: Fn(C) -> O1,
    F2: Fn(D) -> O2,
{
    type Item = (O1, O2);

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|(a, b)| {
            let f1 = self.fn1.as_ref().unwrap();
            let f2 = self.fn2.as_ref().unwrap();
            ((f1)(a), (f2)(b))
        })
    }
}

impl<T: Iterator> ZipPad for T {}

pub trait ZipPad: Iterator {
    fn zip_padded<O>(self, other: O) -> ZipPadded<Self, O::IntoIter>
    where
        Self: Sized,
        O: IntoIterator,
    {
        ZipPadded { iter1: self, iter2: other.into_iter() }
    }
}

pub struct ZipPadded<I1, I2> {
    iter1: I1,
    iter2: I2,
}

impl<I1, I2> Iterator for ZipPadded<I1, I2>
where
    I1: Iterator,
    I2: Iterator,
{
    type Item = (Option<I1::Item>, Option<I2::Item>);

    fn next(&mut self) -> Option<Self::Item> {
        match (self.iter1.next(), self.iter2.next()) {
            (Some(a), Some(b)) => Some((Some(a), Some(b))),
            (Some(a), None) => Some((Some(a), None)),
            (None, Some(b)) => Some((None, Some(b))),
            (None, None) => None,
        }
    }
}
