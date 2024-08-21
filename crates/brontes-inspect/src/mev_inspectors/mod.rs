pub mod atomic_arb;
pub mod cex_dex;

pub mod jit;
pub mod liquidations;
pub mod sandwich;
pub mod searcher_activity;
pub mod shared_utils;

use malachite::Rational;

pub(crate) const MAX_PROFIT: Rational = Rational::const_from_unsigned(15_000_000);
pub(crate) const MIN_PROFIT: Rational = Rational::const_from_unsigned(15_000_000);
