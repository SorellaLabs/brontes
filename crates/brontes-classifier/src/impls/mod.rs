pub mod uniswap;
pub use uniswap::{UniswapV2Classifier, UniswapV3Classifier};

pub mod sushiswap;
pub use sushiswap::{SushiSwapV2Classifier, SushiSwapV3Classifier};

pub mod curve;
pub use curve::CurveCryptoSwapClassifier;

pub mod aave;
pub use aave::AaveV2Classifier;

#[macro_export]
macro_rules! enum_unwrap {
    ($data:ident, $exchange:ident, $return:ident) => {{
        paste::paste! {
            match $data {
                StaticReturnBindings::$exchange(val) => match val {
                    [<$exchange Calls>]::[<$return:lower>](inner) => inner,
                    _ => unreachable!(),
                },
                _ => unreachable!(),
            }
        }
    }};
}
