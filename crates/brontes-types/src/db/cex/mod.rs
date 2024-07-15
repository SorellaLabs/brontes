use std::{fmt::Debug, hash::Hash};

use malachite::Rational;
use strum::Display;
pub mod cex_symbols;
pub mod quotes;
pub mod trades;

pub use cex_symbols::*;
pub use quotes::*;
pub use trades::*;

use crate::pair::Pair;

#[derive(
    Copy,
    Display,
    Debug,
    Clone,
    Default,
    Eq,
    PartialEq,
    Hash,
    serde::Serialize,
    // serde::Deserialize,
    rkyv::Serialize,
    rkyv::Deserialize,
    rkyv::Archive,
)]
#[archive_attr(derive(Eq, PartialEq, Hash))]
pub enum CexExchange {
    Binance,
    Bitmex,
    Deribit,
    Okex,
    Coinbase,
    Kraken,
    BybitSpot,
    Kucoin,
    Upbit,
    Huobi,
    GateIo,
    Bitstamp,
    Gemini,
    Average,
    VWAP,
    OptimisticVWAP,
    #[default]
    Unknown,
}
