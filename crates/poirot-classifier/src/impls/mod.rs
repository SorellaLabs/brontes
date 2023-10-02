pub mod aave_v3;
pub mod uniswap_v2;
pub mod uniswap_v3;

#[macro_export]
macro_rules! enum_unwrap {
    ($data:ident, $exchange:ident, $return:ty) => {{
        use paste::paste;

        unsafe {
            let a = &$data as *const _ as *mut u8;
            let ptr = a.add(2);
            let inner = ptr.cast() as *mut paste!([<$exchange Calls>]);
            let ptr = inner.add(1);

            &*(ptr.cast() as *mut $return)
        }}
    };
}
