use crate::normalized_actions::NormalizedAction;

impl<T: Sized, V: NormalizedAction> ActionIter<V> for T where T: Iterator<Item = V> {}

#[must_use = "iterators are lazy and do nothing unless consumed"]
pub struct FlattenSpecified<V: NormalizedAction, I: Iterator<Item = V>, W, T> {
    iter:      I,
    wanted:    W,
    transform: T,
    extra:     Vec<V>,
}

impl<V: NormalizedAction, I: Iterator<Item = V>, W, T> FlattenSpecified<V, I, W, T> {
    pub(crate) fn new(iter: I, wanted: W, transform: T) -> Self {
        Self { iter, wanted, transform, extra: vec![] }
    }
}

impl<
        V: NormalizedAction,
        R: Clone,
        I: Iterator<Item = V>,
        W: Fn(&V) -> Option<&R>,
        T: Fn(R) -> Vec<V>,
    > Iterator for FlattenSpecified<V, I, W, T>
{
    type Item = V;

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

pub trait ActionIter<V: NormalizedAction>: Iterator<Item = V> {
    fn flatten_specified<R, W, T>(self, wanted: W, transform: T) -> FlattenSpecified<V, Self, W, T>
    where
        Self: Sized,
        R: Clone,
        W: Fn(&V) -> Option<&R>,
        T: Fn(R) -> Vec<V>,
    {
        FlattenSpecified::new(self, wanted, transform)
    }

    fn count_action(self, action: impl Fn(&V) -> bool) -> usize
    where
        Self: Sized,
    {
        let mut i = 0;
        self.into_iter().fold((), |_, x| {
            i += action(&x) as usize;
        });

        i
    }

    fn count_actions<const N: usize>(self, action: [fn(&V) -> bool; N]) -> usize
    where
        Self: Sized,
    {
        let mut i = 0;
        self.into_iter().fold((), |_, x| {
            i += action.iter().any(|ptr| ptr(&x)) as usize;
        });

        i
    }

    fn action_split<FromI, Fns>(self, filters: Fns) -> FromI
    where
        Self: Sized + ActionSplit<FromI, Fns, V>,
    {
        ActionSplit::action_split_impl(self, filters)
    }

    fn action_split_ref<FromI, Fns>(self, filters: &Fns) -> FromI
    where
        Self: Sized + ActionSplit<FromI, Fns, V>,
    {
        ActionSplit::action_split_ref_impl(self, filters)
    }

    fn action_split_out<FromI, Fns>(self, filters: Fns) -> (FromI, Vec<V>)
    where
        Self: Sized + ActionSplit<FromI, Fns, V>,
    {
        ActionSplit::action_split_out_impl(self, filters)
    }

    fn action_split_out_ref<FromI, Fns>(self, filters: &Fns) -> (FromI, Vec<V>)
    where
        Self: Sized + ActionSplit<FromI, Fns, V>,
    {
        ActionSplit::action_split_out_ref_impl(self, filters)
    }

    fn collect_action_vec<R>(self, filter: fn(V) -> Option<R>) -> Vec<R>
    where
        Self: Sized,
    {
        let (low, _) = self.size_hint();
        self.into_iter()
            .fold(Vec::with_capacity(low), |mut acc, x| {
                if let Some(valid) = filter(x) {
                    acc.push(valid)
                }
                acc
            })
    }

    fn collect_action<R, I: Default + Extend<R>>(self, filter: impl Fn(V) -> Option<R>) -> I
    where
        Self: Sized,
    {
        self.into_iter().fold(I::default(), |mut acc, x| {
            if let Some(valid) = filter(x) {
                acc.extend(std::iter::once(valid))
            }
            acc
        })
    }
}

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
            ActionSplit<($($from,)*), ($($fns,)*), V> for IT
            {

            fn action_split_impl(mut self, mut filters: ($($fns,)*)) -> ($($from,)*) {
                let mut res = ($($from::default(),)*);

                let ($($from,)*) = &mut res;
                let ($($fns,)*) = &mut filters;

                while let Some(next) = self.next() {
                    $(
                        if let Some(item) = ($fns)(next.clone()) {
                            $from.extend(std::iter::once(item));
                            continue
                        }

                    )*
                }

                res
            }

            fn action_split_ref_impl(mut self, mut filters: &($($fns,)*)) -> ($($from,)*) {
                let mut res = ($($from::default(),)*);

                let ($($from,)*) = &mut res;
                let ($($fns,)*) = &mut filters;

                while let Some(next) = self.next() {
                    $(
                        if let Some(item) = ($fns)(next.clone()) {
                            $from.extend(std::iter::once(item));
                            continue
                        }

                    )*
                }

                res
            }

            fn action_split_out_impl(mut self, mut filters: ($($fns,)*)) -> (($($from,)*), Vec<V>) {
                let mut res = ($($from::default(),)*);
                let mut rest = Vec::default();

                let ($($from,)*) = &mut res;
                let ($($fns,)*) = &mut filters;

                while let Some(next) = self.next() {
                    $(
                        if let Some(item) = ($fns)(next.clone()) {
                            $from.extend(std::iter::once(item));
                            continue
                        }

                    )*
                        rest.push(next);
                }

                (res, rest)
            }

            fn action_split_out_ref_impl(mut self, mut filters: &($($fns,)*))
                -> (($($from,)*), Vec<V>) {
                let mut res = ($($from::default(),)*);
                let mut rest = Vec::default();

                let ($($from,)*) = &mut res;
                let ($($fns,)*) = &mut filters;

                while let Some(next) = self.next() {
                    $(
                        if let Some(item) = ($fns)(next.clone()) {
                            $from.extend(std::iter::once(item));
                            continue
                        }

                    )*
                        rest.push(next);
                }

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
action_split!((A, RETA, FA), (B, RETB, FB), (C, RETC, FC), (D, RETD, FD), (E, RETE, FE));
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
