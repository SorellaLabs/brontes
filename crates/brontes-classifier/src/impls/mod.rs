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

#[macro_export]
/// all variants for swap implementation
macro_rules! action_impl_all {
    // everything
    (
     $exchange:ident,
     $impl_type:ident,
     $call_type:ident,
     $exchange_mod:ident,
     $fn:expr
     ) => {
        #[derive(Debug, Default)]
        pub struct $exchange;

        impl IntoAction for $exchange {
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
                    .filter_map(|log| {
                        $impl_type::decode_log(log.topics.iter().map(|h| h.0), &log.data, true).ok()
                    })
                    .collect::<Vec<_>>()
                    .remove(0);

                let return_data = $call_type::abi_decode_returns(&return_data, true).unwrap();
                Actions::$impl_type($fn(index, address, call_data, return_data, log_data))
            }
        }
    };
}

#[macro_export]
macro_rules! action_impl_calldata {
    // calldata
    (
        $exchange:ident,
        $impl_type:ident,
        $call_type:ident,
        $exchange_mod:ident,
        $fn:expr
        ) => {
        #[derive(Debug, Default)]
        pub struct $exchange;

        impl IntoAction for $exchange {
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
                let return_data = $call_type::abi_decode_returns(&return_data, true).unwrap();
                Actions::$impl_type($fn(index, address, call_data, return_data))
            }
        }
    };
}

#[macro_export]
macro_rules! action_impl_log {
    // log
    (
        $exchange:ident,
        $impl_type:ident,
        $call_type:ident,
        $fn:expr
        ) => {
        #[derive(Debug, Default)]
        pub struct $exchange;

        impl IntoAction for $exchange {
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
                let log_data = logs
                    .into_iter()
                    .filter_map(|log| {
                        $impl_type::decode_log(log.topics.iter().map(|h| h.0), &log.data, true).ok()
                    })
                    .collect::<Vec<_>>()
                    .remove(0);
                let return_data = $call_type::abi_decode_returns(&return_data, true).unwrap();
                Actions::$impl_type($fn(index, address, log_data, return_data))
            }
        }
    };
}

#[macro_export]
macro_rules! action_impl_return {
    // log
    (
        $exchange:ident,
        $impl_type:ident,
        $call_type:ident,
        $fn:expr
        ) => {
        #[derive(Debug, Default)]
        pub struct $exchange;

        impl IntoAction for $exchange {
            fn get_signature(&self) -> [u8; 4] {
                $call_type::SELECTOR
            }

            fn decode_trace_data(
                &self,
                index: u64,
                _data: StaticReturnBindings,
                return_data: Bytes,
                address: Address,
                _logs: &Vec<Log>,
            ) -> Actions {
                let return_data = $call_type::abi_decode_returns(&return_data, true).unwrap();
                Actions::$impl_type($fn(index, address, return_data))
            }
        }
    };
}

#[macro_export]
/// all variants for swap implementation
macro_rules! action_impl_all_no_return {
    // everything
    (
     $exchange:ident,
     $impl_type:ident,
     $call_type:ident,
     $exchange_mod:ident,
     $fn:expr
     ) => {
        #[derive(Debug, Default)]
        pub struct $exchange;

        impl IntoAction for $exchange {
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
                    .filter_map(|log| {
                        $impl_type::decode_log(log.topics.iter().map(|h| h.0), &log.data, true).ok()
                    })
                    .collect::<Vec<_>>()
                    .remove(0);

                Actions::$impl_type($fn(index, address, call_data, log_data))
            }
        }
    };
}

#[macro_export]
macro_rules! action_impl_calldata_no_return {
    // calldata
    (
        $exchange:ident,
        $impl_type:ident,
        $call_type:ident,
        $exchange_mod:ident,
        $fn:expr
        ) => {
        #[derive(Debug, Default)]
        pub struct $exchange;

        impl IntoAction for $exchange {
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
                Actions::$impl_type($fn(index, address, call_data))
            }
        }
    };
}

#[macro_export]
macro_rules! action_impl_log_no_return {
    // log
    (
        $exchange:ident,
        $impl_type:ident,
        $call_type:ident,
        $fn:expr
        ) => {
        #[derive(Debug, Default)]
        pub struct $exchange;

        impl IntoAction for $exchange {
            fn get_signature(&self) -> [u8; 4] {
                $call_type::SELECTOR
            }

            fn decode_trace_data(
                &self,
                index: u64,
                _data: StaticReturnBindings,
                _return_data: Bytes,
                address: Address,
                logs: &Vec<Log>,
            ) -> Actions {
                let log_data = logs
                    .into_iter()
                    .filter_map(|log| {
                        $impl_type::decode_log(log.topics.iter().map(|h| h.0), &log.data, true).ok()
                    })
                    .collect::<Vec<_>>()
                    .remove(0);
                Actions::$impl_type($fn(index, address, log_data))
            }
        }
    };
}
