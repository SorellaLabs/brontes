impl<T: Sized> IterExt for T where T: Iterator {}

pub trait IterExt: Iterator {
    fn unzip_either<A, B, FromA, FromB>(mut self) -> (FromA, FromB)
    where
        FromA: Default + Extend<A>,
        FromB: Default + Extend<B>,
        Self: Sized + Iterator<Item = (Option<A>, Option<B>)>,
    {
        let mut a: FromA = Default::default();
        let mut b: FromB = Default::default();

        while let Some(next) = self.next() {
            match next {
                (Some(e), None) => a.extend(vec![e]),
                (None, Some(e)) => b.extend(vec![e]),
                (Some(e), Some(e1)) => {
                    a.extend(vec![e]);
                    b.extend(vec![e1])
                }
                _ => {}
            }
        }

        (a, b)
    }
}
