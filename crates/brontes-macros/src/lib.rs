mod action_classifier;
mod discovery_classifier;
use proc_macro::TokenStream;

#[proc_macro]
/// the action impl macro deals with automatically parsing the data needed for
/// underlying actions. The use is as followed
/// ```rust
/// action_impl!(ExchangeName, NormalizedAction, CallType, [LogType / 's], ExchangeModName, [logs: bool , call_data: bool, return_data: bool])
/// ```
/// The Array of log types are expected to be in the order that they are emitted
/// in. Otherwise the decoding will fail
///
///  ## Examples
/// ```rust
/// action_impl!(
///     V2SwapImpl,
///     Swap,
///     swapCall,
///     [Swap],
///     UniswapV2,
///     logs: true,
///     |index, from_address: Address, target_address: Address, msg_sender: Address, log_data: (Swap)| { <body> });
///
/// action_impl!(
///     V2MintImpl,
///     Mint,
///     mintCall,
///     [Mint],
///     UniswapV2,
///     logs: true,
///     call_data: true,
///     |index,
///      from_address: Address,
///      target_address: Address,
///      msg_sender: Address,
///      call_data: mintCall,
///      log_data: (Mint)|  { <body> });
///
/// |index, from_address, target_address, call_data, return_data, log_data| { <body> }
/// ```
///
/// the fields `call_data`, `return_data` and `log_data` are only put into the
/// closure if specified they are always in this order, for example if you put
///  
///  ```return_data: true```
///  then then the closure would be as followed
///  ```|index, from_address, target_address, return_data|```
///
/// for
///  ```
///  log_data: true,
///  call_data: true
///  ````
///  ```|index, from_address, target_address, return_data, log_data|```
pub fn action_impl(input: TokenStream) -> TokenStream {
    action_classifier::action_impl(input.into())
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

#[proc_macro]
pub fn action_dispatch(input: TokenStream) -> TokenStream {
    action_classifier::action_dispatch(input.into())
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

#[proc_macro]
/// the discovery impl macro deals with automatically parsing the data needed
/// for discoverying new pools. The use is as followed
/// ```ignore
/// discovery_impl!(DecoderName, Path::To::Factory::DeployCall, factory address, Parse Fn)
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
pub fn discovery_dispatch(input: TokenStream) -> TokenStream {
    discovery_classifier::discovery_dispatch(input.into())
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
