#[derive(Debug, Default, Clone, Copy)]
pub enum CexTableFlag {
    Trades,
    Quotes,
    #[default]
    None,
}

#[derive(Debug, Clone, Copy)]
pub enum CexRangeOrArbitrary {
    Range(u64, u64),
    Arbitrary(&'static [u64]),
}
