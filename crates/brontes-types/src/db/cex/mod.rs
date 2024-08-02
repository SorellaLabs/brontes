mod best_cex_per_pair;
mod cex_symbols;
mod exchanges;

pub use best_cex_per_pair::*;
pub use cex_symbols::*;
pub use exchanges::*;

pub mod quotes;
pub mod trades;

use strum::Display;

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
    serde::Deserialize,
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
