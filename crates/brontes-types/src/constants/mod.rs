#[cfg(not(feature = "arbitrum"))]
pub mod mainnet;
#[cfg(not(feature = "arbitrum"))]
pub use mainnet::*;

#[cfg(feature = "arbitrum")]
pub mod arbitrum;
#[cfg(feature = "arbitrum")]
pub use arbitrum::*;
