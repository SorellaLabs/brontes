pub mod best_cex_per_pair;
pub mod cex_symbols;
pub mod exchanges;
pub mod quotes;
pub mod trades;

pub use best_cex_per_pair::*;
pub use cex_symbols::*;
pub use exchanges::*;
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
