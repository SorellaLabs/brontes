use std::collections::VecDeque;

pub trait MergeIter<O, Out>: Iterator
where
    Out: Iterator<Item = O>,
{
    fn merge_iter(self) -> Out;
}

macro_rules! merge_iter {
    ($i:tt, $($v:ident),*) => {
        paste::paste!(
            #[allow(non_snake_case,unused_parens)]
            pub struct [<MergeTo $i>]<I: Iterator<Item = ($($v),*)>, $($v),*, O>
                where
                $(
                    O: From<$v>,
                )*
            {
                iter: I,
                buf: VecDeque<O>
            }

            #[allow(non_snake_case,unused_parens)]
            impl<I: Iterator<Item = ($($v),*)>, $($v),*, O> MergeIter<O,
            [<MergeTo $i>]<I, $($v),*, O>> for I
                where
                $(
                    O: From<$v>,
                )*
            {
                fn merge_iter(self) -> [<MergeTo $i>]<I, $($v),*, O>{
                    [<MergeTo $i>] {
                        iter: self,
                        buf: VecDeque::default()
                    }
                }
            }



            #[allow(non_snake_case,unused_parens)]
            impl<I: Iterator<Item = ($($v),*)>, $($v),*, O> Iterator
                for [<MergeTo $i>]<I, $($v),*, O>
                where
                $(
                    O: From<$v>,
                )*
            {
                type Item = O;

                fn next(&mut self) -> Option<Self::Item> {
                    self.iter.next().and_then(|($($v),*)| {
                        $(
                            self.buf.push_back($v.into());
                        )*
                        self.buf.pop_front()
                    })

                }
            }
        );
    }
}

merge_iter!(1, A);
merge_iter!(2, A, B);
merge_iter!(3, A, B, C);
merge_iter!(4, A, B, C, D);
merge_iter!(5, A, B, C, D, E);
merge_iter!(6, A, B, C, D, E, F);

pub trait MergeInto<Out, Ty, I>
where
    Self: Sized,
{
    fn merge_into(self) -> Out;
}

pub trait MergeIntoUnpadded<Out, Ty, I>
where
    Self: Sized,
{
    fn merge_into_unpadded(self) -> Out;
}

macro_rules! merge_into {
    ($out:ident, $typ:ident, $($a:ident),*) => {
        #[allow(non_snake_case, unused_variables, trivial_bounds)]
        impl<T, $($a: Into<$typ>,)* $typ, $out: Default + Extend<$typ>>
            MergeInto<$out, $typ, ($($a,)*)> for T
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
    }
}

merge_into!(A, B, C);
merge_into!(A, B, C, D);
merge_into!(A, B, C, D, E);
merge_into!(A, B, C, D, E, F);
merge_into!(A, B, C, D, E, F, G);
merge_into!(A, B, C, D, E, F, G, H);
merge_into!(A, B, C, D, E, F, G, H, I);

macro_rules! merge_into_unpadded {
    ($out:ident, $typ:ident, $($a:ident),*) => {
        #[allow(non_snake_case, unused_variables, trivial_bounds)]
        impl<T, $($a: Into<$typ>,)* $typ, $out: Default + Extend<$typ>>
            MergeIntoUnpadded<$out, $typ, ($(Option<$a>,)*)> for T
            where
                T: Iterator<Item = ($(Option<$a>,)*)> {

            fn merge_into_unpadded(self) -> $out {
                let mut res = $out::default();
                self.fold((), |(), ($($a,)*)| {
                    $(
                        if let Some(a) = $a {
                            res.extend(std::iter::once(a.into()));
                        }
                    )*

                });

                res
            }
        }
    }
}

merge_into_unpadded!(A, B, C);
merge_into_unpadded!(A, B, C, D);
merge_into_unpadded!(A, B, C, D, E);
merge_into_unpadded!(A, B, C, D, E, F);
merge_into_unpadded!(A, B, C, D, E, F, G);
merge_into_unpadded!(A, B, C, D, E, F, G, H);
merge_into_unpadded!(A, B, C, D, E, F, G, H, I);
