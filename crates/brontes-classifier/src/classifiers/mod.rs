pub mod uniswap;
pub use uniswap::*;

pub mod sushiswap;
pub use sushiswap::*;

pub mod curve;
pub use curve::*;

pub mod aave;
pub use aave::*;

#[macro_export]
macro_rules! enum_unwrap {
    ($data:ident, $exchange:ident, $return:ident) => {{
        paste::paste! {
            match $data {
                crate::StaticReturnBindings::$exchange(val) => match val {
                    crate::$exchange::[<$exchange Calls>]::[<$return>](inner) => inner,
                    _ => unreachable!(),
                },
                _ => unreachable!(),
            }
        }
    }};
}

use brontes_macros::discovery_dispatch;
discovery_dispatch!(
    DiscoveryProtocols,
    SushiSwapV2Decoder,
    SushiSwapV3Decoder,
    UniswapV2Decoder,
    UniswapV3Decoder
);
