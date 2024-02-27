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
