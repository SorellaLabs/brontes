use crate::normalized_actions::NormalizedAction;

pub trait ActionSplit<FromI, Fns, V: NormalizedAction, In> {
    fn action_split_impl(self, filters: Fns) -> FromI;
    fn action_split_ref_impl(self, filters: &Fns) -> FromI;
    fn action_split_out_impl(self, filters: Fns) -> (FromI, Vec<V>);
    fn action_split_out_ref_impl(self, filters: &Fns) -> (FromI, Vec<V>);
}


//TODO: see if there's a good way to handle action reference variants for
// cloning
macro_rules! action_split {
    ($(($fns:ident, $ret:ident, $from:ident, $u:ident)),*) => {
        #[allow(non_snake_case, unused_variables, trivial_bounds, unused_parens)]
        impl <V:NormalizedAction, $($u,)* IT: Iterator<Item = ($($u),*)>,$($ret,)* $($fns: Fn(V) -> Option<$ret>,)*
             $($from: Default + Extend<$ret>),* >
            ActionSplit<($($from,)*), ($($fns,)*), V, ($($u),*)> for IT
            where
                $(
                    $u: Into<V>,
                )*
            {

            fn action_split_impl(self, filters: ($($fns,)*)) -> ($($from,)*) {
                let mut res = ($($from::default(),)*);

                let ($($from,)*) = &mut res;
                let ($($fns,)*) = filters;

                self.flat_map(|($($u),*)|{
                    [$($u.into(),)*]
                }).fold((), |(), item: V| {
                    $(
                        if let Some(item) = ($fns)(item.clone()) {
                            $from.extend(std::iter::once(item));
                            return
                        }

                    )*
                });

                res
            }

            fn action_split_ref_impl(self, mut filters: &($($fns,)*)) -> ($($from,)*) {
                let mut res = ($($from::default(),)*);

                let ($($from,)*) = &mut res;
                let ($($fns,)*) = &mut filters;

                self.flat_map(|($($u),*)|{
                    [$($u.into(),)*]
                })
                .fold((), |(), item:V | {
                    $(
                        if let Some(item) = ($fns)(item.clone()) {
                            $from.extend(std::iter::once(item));
                            return
                        }

                    )*
                });

                res
            }

            fn action_split_out_impl(self, mut filters: ($($fns,)*)) -> (($($from,)*), Vec<V>) {
                let mut res = ($($from::default(),)*);
                let mut rest = Vec::default();

                let ($($from,)*) = &mut res;
                let ($($fns,)*) = &mut filters;

                self.flat_map(|($($u),*)|{
                    [$($u.into(),)*]
                })
                .fold((), |(), item: V| {
                    $(
                        if let Some(item) = ($fns)(item.clone()) {
                            $from.extend(std::iter::once(item));
                            return
                        }
                    )*
                    rest.push(item);
                });

                (res, rest)
            }

            fn action_split_out_ref_impl(self, mut filters: &($($fns,)*))
                -> (($($from,)*), Vec<V>) {
                let mut res = ($($from::default(),)*);

                let mut rest = Vec::default();

                let ($($from,)*) = &mut res;
                let ($($fns,)*) = &mut filters;

                self.flat_map(|($($u),*)|{
                    [$($u.into(),)*]
                })
                .fold((), |(), item:V | {
                    $(
                        if let Some(item) = ($fns)(item.clone()) {
                            $from.extend(std::iter::once(item));
                            return
                        }
                    )*
                    rest.push(item);
                });

                (res, rest)
            }
        }
    };
}

action_split!();
action_split!((A, RETA, FA, AA));
action_split!((A, RETA, FA, AA), (B, RETB, FB, BB));
action_split!((A, RETA, FA, AA), (B, RETB, FB, BB), (C, RETC, FC, CC));
action_split!((A, RETA, FA, AA), (B, RETB, FB, BB), (C, RETC, FC, CC), (D, RETD, FD, DD));
action_split!(
    (A, RETA, FA, AA),
    (B, RETB, FB, BB),
    (C, RETC, FC, CC),
    (D, RETD, FD, DD),
    (E, RETE, FE, EE)
);
