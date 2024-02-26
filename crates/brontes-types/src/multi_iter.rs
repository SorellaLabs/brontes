use std::marker::PhantomData;

fn test_beast(items: Vec<(u8, String)>) {
    let a = items
        .into_iter()
        .map_multisplit()
        .map_item_1(|a| a as u32)
        .map_item_2(|b| b)
        .map(|b| b);
}

pub trait IntoSplitIter {
    type Item;
    type Out;
    type SplitIter: SplitIter<Self::Item, Self::Out>;

    fn into_split_iter(self) -> Self::SplitIter;
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
