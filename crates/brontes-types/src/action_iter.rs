use crate::normalized_actions::Actions;

impl<T: Sized> ActionIter for T where T: Iterator<Item = Actions> {}

pub trait ActionIter: Iterator<Item = Actions> {
    fn action_unzip<FromI, Fns>(self, filters: Fns) -> FromI
    where
        Self: Sized + ActionSplit<FromI, Fns>,
    {
        ActionSplit::action_unzip(self, filters)
    }
}

pub trait ActionSplit<FromI, Fns>: Iterator<Item = Actions> {
    fn action_unzip(self, filters: Fns) -> FromI;
}

//TODO: see if there's a good way to handle action reference variants for
// cloning
macro_rules! action_split {
    ($(($fns:ident, $ret:ident, $from:ident)),*) => {
        #[allow(non_snake_case)]
        impl <IT: Iterator<Item = Actions>,$($ret,)* $($fns: Fn(Actions) -> Option<$ret>),*
            , $($from: Default + Extend<$ret>),* >
            ActionSplit<($($from,)*), ($($fns,)*)> for IT {
            fn action_unzip(mut self, mut filters: ($($fns,)*)) -> ($($from,)*) {
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
