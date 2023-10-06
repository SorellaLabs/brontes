pub mod uniswap_v2;
pub mod uniswap_v3;

#[macro_export]
macro_rules! enum_unwrap {
    ($data:ident, $exchange:ident, $return:ty) => {{
        unsafe {
            let a = &$data as *const _ as *mut u8;
            let ptr = a.add(2);
            let inner = ptr.cast() as *mut $exchange;
            let ptr = inner.add(1);

            &*(ptr.cast() as *mut $return)
        }
    }};
}

// pub trait ActionCollection {
//     fn dispatch(
//         &self,
//         sig: [u8; 4],
//         index: u64,
//         data: StaticReturnBindings,
//         return_data: Bytes,
//         from_address: Address,
//         target_address: Address,
//         logs: &Vec<Log>,
//     ) -> Actions;
// }
#[macro_export]
macro_rules! varient_dispatch {
    ($struct_name:ident, $($name:ident),*) => {
        #[derive(default)]
        pub struct $struct_name {}

        impl ActionCollection for $struct_name {
            //     fn dispatch(
            //         &self,
            //         sig: [u8; 4],
            //         index: u64,
            //         data: StaticReturnBindings,
            //         return_data: Bytes,
            //         from_address: Address,
            //         target_address: Address,
            //         logs: &Vec<Log>,
            //     ) -> Actions;
        }
    };
}
