pub mod uniswap_v2;
pub mod uniswap_v3;
pub use uniswap_v2::{SushiSwapV2Classifier, UniswapV2Classifier};
pub use uniswap_v3::UniswapV3Classifier;

#[macro_export]
macro_rules! enum_unwrap {
    ($data:ident, $exchange:ident, $return:ty) => {{
        /*
                match $data {
                    StaticReturnBindings::$exchange(val) => {
                        match val {
                            $exchange::$return,
                            _ => unreachable!("2nd layer no"),
                        }
                    },
                    _ => unreachable!("1st layer no"),
                }

        */

        unsafe {
            let a = &$data as *const _ as *mut u8;
            let ptr = a.add(4);
            let inner = ptr.cast() as *mut $exchange;
            let ptr = inner.add(1);

            &*(ptr.cast() as *mut $return)
        }
    }};
}
