use crate::normalized_actions::NormalizedAction;

pub trait ActionSplit<FromI, Fns, V: NormalizedAction>: Iterator<Item = V> {
    fn action_split_impl(self, filters: Fns) -> FromI;
    fn action_split_ref_impl(self, filters: &Fns) -> FromI;
    fn action_split_out_impl(self, filters: Fns) -> (FromI, Vec<V>);
    fn action_split_out_ref_impl(self, filters: &Fns) -> (FromI, Vec<V>);
}

//TODO: see if there's a good way to handle action reference variants for
// cloning
macro_rules! action_split {
    ($(($fns:ident, $ret:ident, $from:ident)),*) => {
        #[allow(non_snake_case, unused_variables, trivial_bounds)]
        impl <V:NormalizedAction, IT: Iterator<Item = V>,$($ret,)* $($fns: Fn(V) -> Option<$ret>,)*
             $($from: Default + Extend<$ret>),* >
            ActionSplit<($($from),*), ($($fns),*), V> for IT
            {

            fn action_split_impl(self, filters: ($($fns),*)) -> ($($from),*) {
                let mut res = ($($from::default()),*);

                let ($($from),*) = &mut res;
                let ($($fns),*) = filters;

                self.fold((), |(), item| {
                    $(
                        if let Some(item) = ($fns)(item.clone()) {
                            $from.extend(std::iter::once(item));
                            return
                        }

                    )*
                });

                res
            }

            fn action_split_ref_impl(self, filters: &($($fns),*)) -> ($($from),*) {
                let mut res = ($($from::default()),*);

                let ($($from),*) = &mut res;
                let ($($fns),*) = filters;

                self.fold((), |(), item| {
                    $(
                        if let Some(item) = ($fns)(item.clone()) {
                            $from.extend(std::iter::once(item));
                            return
                        }

                    )*
                });

                res
            }

            fn action_split_out_impl(self, filters: ($($fns),*)) -> (($($from),*), Vec<V>) {
                let mut rest = Vec::default();
                let mut res = ($($from::default()),*);

                let ($($from),*) = &mut res;
                let ($($fns),*) = filters;

                self.fold((), |(), item| {
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

            fn action_split_out_ref_impl(self, filters: &($($fns),*))
                -> (($($from),*), Vec<V>) {
                let mut rest = Vec::default();
                let mut res = ($($from::default()),*);

                let ($($from),*) = &mut res;
                let ($($fns),*) = filters;

                self.fold((), |(), item| {
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
action_split!((A, RETA, FA));
action_split!((A, RETA, FA), (B, RETB, FB));
action_split!((A, RETA, FA), (B, RETB, FB), (C, RETC, FC));
action_split!((A, RETA, FA), (B, RETB, FB), (C, RETC, FC), (D, RETD, FD));
action_split!(
    (A, RETA, FA),
    (B, RETB, FB),
    (C, RETC, FC),
    (D, RETD, FD),
    (E, RETE, FE)
);
action_split!(
    (A, RETA, FA),
    (B, RETB, FB),
    (C, RETC, FC),
    (D, RETD, FD),
    (E, RETE, FE),
    (F, RETF, FF)
);
action_split!(
    (A, RETA, FA),
    (B, RETB, FB),
    (C, RETC, FC),
    (D, RETD, FD),
    (E, RETE, FE),
    (F, RETF, FF),
    (G, RETG, FG)
);
