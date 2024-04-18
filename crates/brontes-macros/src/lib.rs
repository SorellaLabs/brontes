mod action_classifier;
mod discovery_classifier;
mod libmdbx_test;
mod bench_struct_methods;

use proc_macro::TokenStream;
use syn::{parse_macro_input, ItemFn};

use crate::action_classifier::{ActionDispatch, ActionMacro};

#[proc_macro]
/// the action impl macro deals with automatically parsing the data needed for
/// underlying actions. The use is as followed
/// ```ignore
/// action_impl!(ProtocolPath, PathToCall, CallType, [LogType / 's], [logs: bool , call_data: bool, return_data: bool])
/// ```
/// The generated structs name will be as the following:
///  &lt;LastIdentInProtocolPath&gt; + &lt;LastIdentInPathToCall&gt;
/// Example:
/// a macro invoked with
///     Protocol::UniswapV2,
///     crate::UniswapV2::swapCall,
///
/// becomes: UniswapV2swapCall.
/// This is done to avoid naming conflicts between classifiers as this is name
/// will always be unique.
///
/// The Array of log types are expected to be in the order that they are emitted
/// in. Otherwise the decoding will fail
///
///  ## Examples
/// ```ignore
/// action_impl!(
///     Protocol::UniswapV2,
///     crate::UniswapV2::swapCall,
///     Swap,
///     [..Swap],
///     logs: true,
///     |index,
///     from_address: Address,
///     target_address: Address,
///     msg_sender: Address,
///     log_data: UniswapV2swapCallLogs| { <body> });
///
/// action_impl!(
///     Protocol::UniswapV2,
///     crate::UniswapV2::mintCall,
///     Mint,
///     [..Mint],
///     logs: true,
///     call_data: true,
///     |index,
///      from_address: Address,
///      target_address: Address,
///      msg_sender: Address,
///      call_data: mintCall,
///      log_data: UniswapV2mintCallLogs|  { <body> });
/// ```
///
/// # Logs Config
/// NOTE: all log modifiers are compatible with each_other
/// ## Log Ignore Before
/// if you want to ignore all logs that occurred before a certain log,
/// prefix the log with .. ex `..Mint`.
///
/// ## Log Repeating
/// if a log is repeating and dynamic in length, use `*` after the log
/// to mark that there is a arbitrary amount of these logs emitted.
/// ex `Transfer*` or `..Transfer*`
///
/// ## Fallback logs.
/// in the case that you might need a fallback log, these can be defined by
/// wrapping the names in parens. e.g (Transfer | SpecialTransfer).
/// this will try to decode transfer first and if it fails, special transfer.
/// Fallback logs are configurable with other log parsing options. this means
/// you can do something like ..(Transfer | SpecialTransfer) or ..(Transfer |
/// SpecialTransfer)*
///
///
/// the fields `call_data`, `return_data` and `log_data` are only put into the
/// closure if specified they are always in this order, for example if you put
///  
///  ```return_data: true```
///  then then the closure would be as followed
///  ```|index, from_address, target_address, return_data|```
///
/// for
///  ```ignore
///  log_data: true,
///  call_data: true
///  ````
///  ```|index, from_address, target_address, return_data, log_data|```
pub fn action_impl(input: TokenStream) -> TokenStream {
    parse_macro_input!(input as ActionMacro)
        .expand()
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

#[proc_macro]
/// action_dispatch macro crates a struct that automatically dispatches
/// the given trace information to the proper action classifier. its invoked as
/// the following:
/// ```ignore
/// action_dispatch!(<DispatchStructName>, [action_classifier_names..],);
/// ```
/// an actual example would be
/// ```ignore
/// # use brontes_macros::{action_dispatch, action_impl};
/// # use brontes_pricing::Protocol;
/// # use brontes_types::normalized_actions::NormalizedSwap;
/// # use alloy_primitives::Address;
/// # use brontes_database::libmdbx::tx::CompressedLibmdbxTx;
///
/// action_impl!(
///     Protocol::UniswapV2,
///     crate::UniswapV2::swapCall,
///     Swap,
///     [Ignore<Sync>, Swap],
///     call_data: true,
///     logs: true,
///     |trace_index,
///     from_address: Address,
///     target_address: Address,
///      msg_sender: Address,
///     call_data: swapCall,
///     log_data: UniswapV2swapCallLogs,
///     db_tx: &DB| {
///         todo!()
///     }
/// );
///
/// action_dispatch!(ClassifierDispatch, UniswapV2swapCall);
/// ```
pub fn action_dispatch(input: TokenStream) -> TokenStream {
    parse_macro_input!(input as ActionDispatch)
        .expand()
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

#[proc_macro]
/// the discovery impl macro deals with automatically parsing the data needed
/// for discoverying new pools. The use is as followed
/// ```ignore
/// discovery_impl!(DiscoveryName, Path::To::Factory::DeployCall, factory address, Parse Fn);
/// ```
/// where Parse Fn
/// ```ignore
/// |deployed_address: Address, decoded_call_data: DeployCall, provider: Arc<T>| { <body> }
/// ```
pub fn discovery_impl(input: TokenStream) -> TokenStream {
    discovery_classifier::discovery_impl(input.into())
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

#[proc_macro]
/// # Eth Curve Pool Discovery
/// Curve is wierd since each factory contract (7 of them) has multiple
/// implementations of each create base/plain/meta pool, so it has it's own impl
/// ### Fields
/// 1. `Protocol` (enum in types) - Curve version
/// 2. Path to the `sol!` generated abi for the factory
/// 3. `x` concatenated with the factory address
/// 4. A tuple with the fields (x, y, z)
///     - x: number of base pools
///     - y: number of metapools
///     - z: number of plain pools
///
/// ### Example
/// ```ignore
/// curve_discovery_impl!(
///     CurvecrvUSD,
///     crate::raw::pools::impls::CurvecrvUSDFactory,
///     x4f8846ae9380b90d2e71d5e3d042dff3e7ebb40d,
///     (1, 2, 3)
/// );
/// ```
pub fn curve_discovery_impl(input: TokenStream) -> TokenStream {
    discovery_classifier::curve::curve_discovery_impl(input.into())
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

#[proc_macro]
/// discovery dispatch macro creates a struct that automatically dispatches
/// possible CREATE traces to the proper discovery classifier
/// ```ignore
/// discovery_dispatch!(<DispatchStructName>, [discovery_impl_name..],);
/// ```
pub fn discovery_dispatch(input: TokenStream) -> TokenStream {
    discovery_classifier::discovery_dispatch(input.into())
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

#[proc_macro_attribute]
pub fn test(attr: TokenStream, item: TokenStream) -> TokenStream {
    let item = parse_macro_input!(item as ItemFn);
    libmdbx_test::parse(item, attr.into())
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

#[proc_macro_attribute]
pub fn bench_time(attr: TokenStream, item: TokenStream) -> TokenStream {
    let item = parse_macro_input!(item as ItemFn);
    bench_struct_methods::parse(item, attr.into())
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
