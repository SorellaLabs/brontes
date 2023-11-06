pub mod uniswap_v2;
pub mod uniswap_v3;
pub use uniswap_v2::{SushiSwapV2Classifier, UniswapV2Classifier};
pub use uniswap_v3::UniswapV3Classifier;

#[macro_export]
macro_rules! enum_unwrap {
    ($data:ident, $exchange:ident, $return_path:ident) => {{
        enum_unwrap!(@$data, $exchange, paste::paste!([<$exchange Calls>])::$return_path)
    }};

    (@ $data:ident, $exchange:ident, $return_path:path) => {{
        match $data {
            StaticReturnBindings::$exchange(val) => match val {
                $return_path(inner) => inner,
                _ => unreachable!("2nd layer no"),
            },
            _ => unreachable!("1st layer no"),
        }
    }};
}
