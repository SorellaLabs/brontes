use std::marker::PhantomData;

pub trait IntoSplitIterator {
    type Item;
    type Iter: Iterator<Item = Self::Item>;

    fn into_split_iter(self) -> Self::Iter;
}

pub trait SplitIter<Item, K>: Iterator<Item = Item> {
    fn multisplit_builder(self) -> K;
}

pub trait SplitBuilder {}

impl<I, A, B, F1, F2> SplitIter<(A, B), SplitIterTwo<A, B, I, false, false, F1, F2>> for I
where
    I: Iterator<Item = (A, B)>,
{
    fn multisplit_builder(self) -> SplitIterTwo<A, B, I, false, false, F1, F2> {
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

macro_rules! into_split_iter {
    ($am:tt, $($iter_val:ident),*) => {
        paste::paste!(

            impl<$($iter_val),*> IntoSplitIterator for ($($iter_val,)*)
            where
                $(
                    $iter_val: IntoIterator,
                )*
            {
                type Item = ($(Option<$iter_val::Item>,)*);
                type Iter = [<ZipPadded $am>]<$($iter_val::IntoIter),*>;

                fn into_split_iter(self) -> Self::Iter {
                    let ($([<$iter_val:lower>],)*) = self;

                    [<ZipPadded $am>] {
                        $(
                            [<$iter_val:lower>]: [<$iter_val:lower>].into_iter(),
                        )*
                    }
                }
            }

            #[derive(Clone)]
            pub struct [<ZipPadded $am>]<$($iter_val),*> {
                $(
                    [<$iter_val:lower>]: $iter_val,
                )*
            }

            impl<$($iter_val),*> Iterator for [<ZipPadded $am>]<$($iter_val),*>
            where
                $(
                    $iter_val: Iterator,
                )* {
                    type Item = ($(Option<$iter_val::Item>,)*);

                    fn next(&mut self) -> Option<Self::Item> {
                        let mut all_none = true;
                        $(
                            let mut [<$iter_val:lower>] = None::<$iter_val::Item>;
                        )*

                        $(
                            if let Some(val) = self.[<$iter_val:lower>].next() {
                                all_none = false;
                                [<$iter_val:lower>] = Some(val);
                            }
                        )*

                        if all_none {
                            return None
                        }

                        Some(($([<$iter_val:lower>],)*))
                    }
                }
            );

    };
}

into_split_iter!(1, A);
into_split_iter!(2, A, B);
into_split_iter!(3, A, B, C);
into_split_iter!(4, A, B, C, D);
into_split_iter!(5, A, B, C, D, E);
into_split_iter!(6, A, B, C, D, E, F);
into_split_iter!(7, A, B, C, D, E, F, G);
into_split_iter!(8, A, B, C, D, E, F, G, H);
