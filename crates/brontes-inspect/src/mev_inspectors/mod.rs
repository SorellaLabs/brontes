pub mod atomic_arb;
pub mod cex_dex;

pub mod jit;
pub mod liquidations;
pub mod sandwich;
pub mod searcher_activity;
pub mod shared_utils;

use malachite::Rational;
/// anything more than this in profit is most likely a false_positive
pub(crate) const MAX_PROFIT: Rational = Rational::const_from_unsigned(50_000_000);
