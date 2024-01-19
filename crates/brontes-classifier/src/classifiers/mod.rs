pub mod uniswap;
pub use uniswap::{UniswapDecoder, UniswapV2Classifier, UniswapV3Classifier, UniswapXClassifier};

pub mod sushiswap;
pub use sushiswap::{SushiSwapV2Classifier, SushiSwapV3Classifier};

pub mod curve;
pub use curve::{CurveCryptoSwapClassifier, CurveDecoder};

pub mod aave;
pub use aave::{AaveV2Classifier, AaveV3Classifier};

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
