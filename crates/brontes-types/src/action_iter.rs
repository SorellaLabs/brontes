use crate::{
    normalized_actions::NormalizedAction,
    tree::{ActionSplit, FlattenSpecified},
};

impl<T: Sized, V: NormalizedAction> ActionIter<V> for T where T: Iterator<Item = V> {}
pub trait ActionIter<V: NormalizedAction>: Iterator<Item = V> {
    fn flatten_specified<R, W, T>(self, wanted: W, transform: T) -> FlattenSpecified<V, Self, W, T>
    where
        Self: Sized,
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
        Self: Sized + ActionSplit<FromI, Fns, V, Self::Item>,
    {
        ActionSplit::action_split_impl(self, filters)
    }

    fn action_split_ref<FromI, Fns>(self, filters: &Fns) -> FromI
    where
        Self: Sized + ActionSplit<FromI, Fns, V, Self::Item>,
    {
        ActionSplit::action_split_ref_impl(self, filters)
    }

    fn action_split_out<FromI, Fns>(self, filters: Fns) -> (FromI, Vec<V>)
    where
        Self: Sized + ActionSplit<FromI, Fns, V, Self::Item>,
    {
        ActionSplit::action_split_out_impl(self, filters)
    }

    fn action_split_out_ref<FromI, Fns>(self, filters: &Fns) -> (FromI, Vec<V>)
    where
        Self: Sized + ActionSplit<FromI, Fns, V, Self::Item>,
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
