#[derive(Debug, Default, Clone, Copy)]
pub enum CexTableFlag {
    Trades,
    Quotes,
    #[default]
    None,
}

// all of this info is in timestamps
#[derive(Debug, Clone, Copy)]
pub enum CexRangeOrArbitrary {
    Range(u64, u64),
    Arbitrary(&'static [u64]),
    Timestamp { block_number: u64, block_timestamp: u64 },
}
