use std::sync::Arc;

use super::TreeIter;
use crate::{normalized_actions::NormalizedAction, BlockTree};

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

pub trait IntoZip<Out> {
    fn into_zip(self) -> Out;
}

pub trait IntoZipTree<V: NormalizedAction, Out> {
    fn into_zip_tree(self, tree: Arc<BlockTree<V>>) -> Out;
}

unzip_padded!((A, A1));
unzip_padded!((A, A1), (B, B1));
unzip_padded!((A, A1), (B, B1), (C, C1));
unzip_padded!((A, A1), (B, B1), (C, C1), (D, D1));
unzip_padded!((A, A1), (B, B1), (C, C1), (D, D1), (E, E1));

macro_rules! into_split_iter {
    ($am:tt $am1:tt, $($iter_val:ident),*) => {
        paste::paste!(
            into_split_iter!($am, $($iter_val),*);

            impl<$($iter_val),*> IntoZip<[<ZipPadded $am>]<$($iter_val::IntoIter),*>> for ($($iter_val),*)
            where
                $(
                    $iter_val: IntoIterator
                ),*
            {
                fn into_zip(self) -> [<ZipPadded $am>]<$($iter_val::IntoIter),*> {
                    let ($([<$iter_val:lower>]),*) = self;

                    [<ZipPadded $am>] {
                        $(
                            [<$iter_val:lower>]: [<$iter_val:lower>].into_iter(),
                        )*
                    }
                }
            }

            impl<V: NormalizedAction, $($iter_val),*> IntoZipTree<V,[<ZipPaddedTree $am>]<V,$($iter_val::IntoIter),*>> for ($($iter_val),*)
            where
                $(
                    $iter_val: IntoIterator
                ),*
            {
                fn into_zip_tree(self, tree: Arc<BlockTree<V>>)
                    -> [<ZipPaddedTree $am>]<V, $($iter_val::IntoIter),*> {
                    let ($([<$iter_val:lower>]),*) = self;

                    [<ZipPaddedTree $am>] {
                        tree,
                        $(
                            [<$iter_val:lower>]: [<$iter_val:lower>].into_iter(),
                        )*
                    }
                }
            }

            impl<V:NormalizedAction, $($iter_val),*, I> SplitIterZip<I>
                for [<ZipPaddedTree $am>]<V,$($iter_val),*>
                where
                    I: Iterator,
                $(
                    $iter_val: Iterator,
                )* {

                type Out = [<ZipPaddedTree $am1>]<V,$($iter_val),*, I>;

                fn zip_with_inner(self, other: I) -> Self::Out
                {
                    [<ZipPaddedTree $am1>]::new(self.tree, $(self.[<$iter_val:lower>],)* other)
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
        );
    };
    ($am:tt, $($iter_val:ident),*) => {
        paste::paste!(

            #[derive(Clone)]
            pub struct [<ZipPaddedTree $am>]<V: NormalizedAction, $($iter_val),*> {
                tree: Arc<BlockTree<V>>,
                $(
                    [<$iter_val:lower>]: $iter_val,
                )*
            }

            impl <V: NormalizedAction, $($iter_val),*> [<ZipPaddedTree $am>]<V,$($iter_val),*> {
                pub fn new(
                    tree: Arc<BlockTree<V>>,
                $(
                    [<$iter_val:lower>]: $iter_val,
                )*) -> Self {
                    Self {
                        tree,
                        $([<$iter_val:lower>]),*
                    }
                }
            }

            impl<V: NormalizedAction,$($iter_val),*> TreeIter<V> for [<ZipPaddedTree $am>]<V,$($iter_val),*> {
                fn tree(&self) -> Arc<BlockTree<V>> {
                    self.tree.clone()
                }
            }

            impl<V: NormalizedAction, $($iter_val),*> Iterator for [<ZipPaddedTree $am>]<V,$($iter_val),*>
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


            #[derive(Clone)]
            pub struct [<ZipPadded $am>]<$($iter_val),*> {
                $(
                    [<$iter_val:lower>]: $iter_val,
                )*
            }

            impl <$($iter_val),*> [<ZipPadded $am>]< $($iter_val),*> {
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
