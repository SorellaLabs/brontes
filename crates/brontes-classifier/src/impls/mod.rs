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

#[macro_export]
/// all variants for swap implementation
macro_rules! action_impl{
    // everything
    (
     $exchange:ident,
     $impl_type:ident,
     $call_type:ident,
     $exchange_mod:ident,
     $event_struct:ident,
     $return_calldata:expr,
     $fn:expr
     ) => {
        pub struct paste![<$exchange $impl_type>];

        impl IntoAction for paste![<$exchange $impl_type>] {
            fn get_signature(&self) -> [u8; 4] {
                $call_type::SELECTOR
            }

            fn decode_trace_data(
                &self,
                index: u64,
                data: StaticReturnBindings,
                return_data: Bytes,
                address: Address,
                logs: &Vec<Log>,
            ) -> Actions {
                let call_data = enum_unwrap!(data, $exchange_mod, $call_type);
                let log_data = logs
                    .into_iter()
                    .filter_map(|log|$event_struct::decode_log(log.
                                                                topics
                                                                .iter()
                                                                .map(|h| h.0)
                                                                , &log.data,
                                                                true).ok())
                    .collect::<Vec<_>>().first().unwrap();

                if $return_calldata {
                    let return_data = $call_type::abi_decode_returns(&return_data, true).unwrap();
                    Actions::$impl_type($fn(index, address, call_data, return_data, log_data))
                } else {
                    Actions::$impl_type($fn(index, address, call_data, log_data))
                }
            }
        }
    };
    // calldata
    (
        $exchange:ident,
        $impl_type:ident,
        $call_type:ident,
        $exchange_mod:ident,
        $return_calldata:expr,
        $fn:expr
        ) => {
        pub struct paste![<$exchange $impl_type>];

        impl IntoAction for paste![<$exchange $impl_type>] {
            fn get_signature(&self) -> [u8; 4] {
                $call_type::SELECTOR
            }

            fn decode_trace_data(
                &self,
                index: u64,
                data: StaticReturnBindings,
                return_data: Bytes,
                address: Address,
                logs: &Vec<Log>,
            ) -> Actions {
                let call_data = enum_unwrap!(data, $exchange_mod, $call_type);
                if $return_calldata  {
                    let return_data = $call_type::abi_decode_returns(&return_data, true).unwrap();
                    Actions::$impl_type($fn(index, address, call_data, return_data))
                } else {
                    Actions::$impl_types($fn(index, address, call_data))
                }
            }
        }
    };

    // return data
    (
        $exchange:ident,
        $impl_type:ident,
        $call_type:ident,
        $event_struct:ident,
        $return_calldata:expr,
        $fn:expr
        ) => {
        pub struct paste![<$exchange $impl_type>];

        impl IntoAction for paste![<$exchange $impl_type>] {
            fn get_signature(&self) -> [u8; 4] {
                $call_type::SELECTOR
            }

            fn decode_trace_data(
                &self,
                index: u64,
                data: StaticReturnBindings,
                return_data: Bytes,
                address: Address,
                logs: &Vec<Log>,
            ) -> Actions {
                let return_data = $call_type::abi_decode_returns(&return_data, true).unwrap();
                let log_data = logs
                    .into_iter()
                    .filter_map(|log|$event_struct::decode_log(log.
                                                                topics
                                                                .iter()
                                                                .map(|h| h.0),
                                                                &log.data,
                                                                true).ok())
                    .collect::<Vec<_>>().first().unwrap();
                if $return_calldata  {
                    let return_data = $call_type::abi_decode_returns(&return_data, true).unwrap();
                    Actions::$impl_type($fn(index, address, return_data, log_data))
                } else {
                    Actions::$impl_type($fn(index, address, return_data))
                }
            }
        }
    };
}
