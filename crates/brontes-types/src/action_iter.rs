use crate::normalized_actions::Actions;

impl<T: Sized> ActionIter for T where T: Iterator<Item = Actions> {}

pub struct FlattenSpecified<I: Iterator<Item = Actions>, W, T> {
    iter:      I,
    wanted:    W,
    transform: T,
    extra:     Vec<Actions>,
}
impl<I: Iterator<Item = Actions>, W, T> FlattenSpecified<I, W, T> {
    pub(crate) fn new(iter: I, wanted: W, transform: T) -> Self {
        Self { iter, wanted, transform, extra: vec![] }
    }
}

impl<
        R: Clone,
        I: Iterator<Item = Actions>,
        W: Fn(&Actions) -> Option<&R>,
        T: Fn(R) -> Vec<Actions>,
    > Iterator for FlattenSpecified<I, W, T>
{
    type Item = Actions;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(extra) = self.extra.pop() {
            return Some(extra)
        }

        self.iter
            .next()
            .map(|item| {
                if let Some(wanted) = (self.wanted)(&item) {
                    let mut ret = (self.transform)(wanted.clone());
                    let now = ret.pop();
                    self.extra.extend(ret);
                    now
                } else {
                    Some(item)
                }
            })
            .flatten()
    }
}

pub trait ActionIter: Iterator<Item = Actions> {
    fn flatten_specified<R, W, T>(self, wanted: W, transform: T) -> FlattenSpecified<Self, W, T>
    where
        Self: Sized,
        R: Clone,
        W: Fn(&Actions) -> Option<&R>,
        T: Fn(R) -> Vec<Actions>,
    {
        FlattenSpecified::new(self, wanted, transform)
    }

    fn action_split<FromI, Fns>(self, filters: Fns) -> FromI
    where
        Self: Sized + ActionSplit<FromI, Fns>,
    {
        ActionSplit::action_split(self, filters)
    }

    fn action_split_ref<FromI, Fns>(self, filters: &Fns) -> FromI
    where
        Self: Sized + ActionSplit<FromI, Fns>,
    {
        ActionSplit::action_split_ref(self, filters)
    }

    fn collect_action_vec<R>(self, filter: fn(Actions) -> Option<R>) -> Vec<R>
    where
        Self: Sized,
    {
        let (low, _) = self.size_hint();
        self.fold(Vec::with_capacity(low), |mut acc, x| {
            if let Some(valid) = filter(x) {
                acc.push(valid)
            }
            acc
        })
    }

    fn collect_action<R, I: Default + Extend<R>>(self, filter: impl Fn(Actions) -> Option<R>) -> I
    where
        Self: Sized,
    {
        self.fold(I::default(), |mut acc, x| {
            if let Some(valid) = filter(x) {
                acc.extend(std::iter::once(valid))
            }
            acc
        })
    }
}

pub trait ActionSplit<FromI, Fns>: Iterator<Item = Actions> {
    fn action_split(self, filters: Fns) -> FromI;
    fn action_split_ref(self, filters: &Fns) -> FromI;
}

//TODO: see if there's a good way to handle action reference variants for
// cloning
macro_rules! action_split {
    ($(($fns:ident, $ret:ident, $from:ident)),*) => {
        #[allow(non_snake_case)]
        impl <IT: Iterator<Item = Actions>,$($ret,)* $($fns: Fn(Actions) -> Option<$ret>),*
            , $($from: Default + Extend<$ret>),* >
            ActionSplit<($($from,)*), ($($fns,)*)> for IT {
            fn action_split(mut self, mut filters: ($($fns,)*)) -> ($($from,)*) {
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

            fn action_split_ref(mut self, mut filters: &($($fns,)*)) -> ($($from,)*) {
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
        }
    };
}

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
