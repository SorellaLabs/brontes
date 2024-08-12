use std::{fmt::Debug, hash::Hash};

use strum::Display;
pub mod cex_symbols;
pub mod quotes;
pub mod trades;
mod best_cex_per_pair;
mod exchanges;

pub use best_cex_per_pair::*;
pub use cex_symbols::*;
pub use quotes::*;
pub use trades::*;

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
