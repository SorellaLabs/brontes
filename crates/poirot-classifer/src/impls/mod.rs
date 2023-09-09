pub mod uniswap_v3;

#[macro_export]
macro_rules! yoink_decoded_type{
    ($data:ident, $exchange:ident, $return:ty) => {{
        use paste::paste;

        unsafe {
            let a = &$data as *const _ as *mut u8;
            let ptr = a.add(8);
            let inner = ptr.cast() as *mut paste!([<$exchange Calls>]);
            let ptr = inner.add(8);

            &*(ptr.cast() as *mut $return)
        }}
    };
}
