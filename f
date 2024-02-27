use std::marker::PhantomData;

use itertools::Itertools;

pub trait IntoSplitIterator {
    type Item;
    type Iter: Iterator<Item = Self::Item>;

    fn into_split_iter(self) -> Self::Iter;
}

impl<T: Sized> TreeIterExt for T where T: Iterator {}

pub trait TreeIterExt: Iterator {
    fn zip_with<O>(self, other: O) -> Self::Out
    where
        Self: SplitIterZip<O> + Sized,
        O: Iterator,
    {
        SplitIterZip::<O>::zip_with_inner(self, other)
    }

    fn unzip_padded<FromZ>(self) -> FromZ
    where
        Self: UnzipPadded<FromZ> + Sized,
    {
        UnzipPadded::unzip_padded(self)
    }

    fn fold_using<I, Ty>(self) -> I
    where
        Self: Sized,
        Self: MergeInto<I,Ty,<Self as Iterator>::Item>
    {
        MergeInto::<I, Ty, Self::Item>::merge_into(self)
    }

    // fn merge_into<I, Ty>(self) -> I
    // where
    //     // Self: MergeInto<I, Ty, Self::Item> + Sized,
    // {
    //     MergeInto::merge_into(self)
    // }
}

pub trait MergeInto<Out, Ty, I>
where
    Self: Sized,
{
    fn merge_into(self) -> Out;
}

macro_rules! merge_into {
    ($out:ident, $typ:ident, $($a:ident),*) => {
        #[allow(non_snake_case, unused_variables, trivial_bounds)]
        impl<T, $($a: Into<$typ>,)* $typ, $out: Default + Extend<$typ>> MergeInto<$out, $typ, ($($a,)*)> for T
            where
                T: Iterator<Item = ($($a,)*)> {

            fn merge_into(self) -> $out {
                let mut res = $out::default();
                self.fold((), |(), ($($a,)*)| {
                    $(
                            res.extend(std::iter::once($a.into()));
                    )*

                });

                res
            }
        }
        // #[allow(non_snake_case, unused_variables, trivial_bounds)]
        // impl<T, $($a: Into<$typ>,)* $typ, $out: Default + Extend<$typ>> MergeInto<$out, $typ, ($(Option<$a>,)*)> for T
        //     where
        //         T: Iterator<Item = ($(Option<$a>,)*)> {
        //     fn merge_into(self) -> $out {
        //         let mut res = $out::default();
        //         self.fold((), |(), ($($a,)*)| {
        //             $(
        //                 if let Some(a) = $a {
        //                     res.extend(std::iter::once(a.into()));
        //                 }
        //             )*
        //
        //         });
        //
        //         res
        //     }
        // }
    }
}

merge_into!(A, B, C);
merge_into!(A, B, C, D);
merge_into!(A, B, C, D, E);
merge_into!(A, B, C, D, E, F);
merge_into!(A, B, C, D, E, F, G);
merge_into!(A, B, C, D, E, F, G, H);
merge_into!(A, B, C, D, E, F, G, H, I);

pub trait SplitIterZip<NewI>: Iterator
where
    NewI: Iterator,
{
    type Out: Iterator;

    fn zip_with_inner(self, other: NewI) -> Self::Out;
}

pub trait UnzipPadded<Out>: Iterator {
    fn unzip_padded(self) -> Out;
}

macro_rules! unzip_padded {
    ($(($a:ident, $b:ident)),*) => {
        #[allow(non_snake_case, unused_variables, trivial_bounds)]
        impl <T, $($a,)* $($b: Default + Extend<$a>,)*> UnzipPadded<($($b,)*)> for T
            where
                T: Iterator<Item = ($(Option<$a>,)*)> {

            fn unzip_padded(self) -> ($($b,)*) {
                let ($(mut $b,)*) = ($($b::default(),)*);
                self.fold((), |(), ($($a,)*)| {
                    $(
                        if let Some(a) = $a {
                            $b.extend(std::iter::once(a));
                        }
                    )*
                });

                ($($b,)*)
            }
        }
    };
}

unzip_padded!((A, A1));
unzip_padded!((A, A1), (B, B1));
unzip_padded!((A, A1), (B, B1), (C, C1));
unzip_padded!((A, A1), (B, B1), (C, C1), (D, D1));
unzip_padded!((A, A1), (B, B1), (C, C1), (D, D1), (E, E1));

pub trait SplitIter<Item, K>: Iterator<Item = Item> {
    fn multisplit_builder(self) -> K;
}

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
    ($am:tt $am1:tt, $($iter_val:ident),*) => {
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
            impl <$($iter_val),*> [<ZipPadded $am>]<$($iter_val),*> {
                pub fn new(
                $(
                    [<$iter_val:lower>]: $iter_val,
                )*) -> Self {
                    Self {
                        $([<$iter_val:lower>]),*
                    }
                }

            }

            impl<$($iter_val),*, I> SplitIterZip<I>
                for [<ZipPadded $am>]<$($iter_val),*>
                where
                    I: Iterator,
                $(
                    $iter_val: Iterator,
                )* {

                type Out = [<ZipPadded $am1>]<$($iter_val),*, I>;

                fn zip_with_inner(self, other: I) -> Self::Out
                {
                    [<ZipPadded $am1>]::new($(self.[<$iter_val:lower>],)* other)
                }
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

            impl <$($iter_val),*> [<ZipPadded $am>]<$($iter_val),*> {
                pub fn new(
                $(
                    [<$iter_val:lower>]: $iter_val,
                )*) -> Self {
                    Self {
                        $([<$iter_val:lower>]),*
                    }
                }

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

into_split_iter!(1 2, A);
into_split_iter!(2 3, A, B);
into_split_iter!(3 4, A, B, C);
into_split_iter!(4 5, A, B, C, D);
into_split_iter!(5 6, A, B, C, D, E);
into_split_iter!(6 7, A, B, C, D, E, F);
into_split_iter!(7 8, A, B, C, D, E, F, G);
into_split_iter!(8, A, B, C, D, E, F, G, H);
