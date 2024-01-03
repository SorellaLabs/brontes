mod action_classifier;
mod bench_struct_methods;
mod discovery_classifier;
mod function_metrics;
mod libmdbx_test;
mod transpose;

use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput, ItemFn};

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
pub fn action_impl(token_stream: TokenStream) -> TokenStream {
    let MacroParse {
        exchange_name,
        action_type,
        call_type,
        log_type,
        exchange_mod_name,
        give_logs,
        give_returns,
        call_function,
        give_calldata,
    } = syn::parse2(token_stream.into()).unwrap();

    let mut option_parsing = Vec::new();

    let a = call_type.to_string();
    let decalled = Ident::new(&a[..a.len() - 4], Span::call_site().into());

    if give_calldata {
        option_parsing.push(quote!(
                let call_data = enum_unwrap!(data, #exchange_mod_name, #decalled);
        ));
    }

    if give_logs {
        option_parsing.push(quote!(
            let log_data = logs.into_iter().filter_map(|log| {
                #log_type::decode_log(log.topics.iter().map(|h| h.0), &log.data, false).ok()
            }).collect::<Vec<_>>();
            let log_data = Some(log_data).filter(|data| !data.is_empty()).map(|mut l| l.remove(0));
        ));
    }

    if give_returns {
        option_parsing.push(quote!(
                let return_data = #call_type::abi_decode_returns(&return_data, false).map_err(|e| {
                    tracing::error!("return data failed to decode {:#?}", return_data);
                    e
                }).unwrap();
        ));
    }

    let fn_call = match (give_calldata, give_logs, give_returns) {
        (true, true, true) => {
            quote!(
            (#call_function)(index, from_address, target_address, call_data, return_data, log_data, db_tx)
            )
        }
        (true, true, false) => {
            quote!(
                (#call_function)(index, from_address, target_address, call_data, log_data, db_tx)
            )
        }
        (true, false, true) => {
            quote!(
                (#call_function)(index, from_address, target_address, call_data, return_data, db_tx)
            )
        }
        (true, false, false) => {
            quote!(
                (#call_function)(index, from_address, target_address, call_data, db_tx)
            )
        }
        (false, true, true) => {
            quote!(
                (#call_function)(index, from_address, target_address, return_data, log_data, db_tx)
            )
        }
        (false, false, true) => {
            quote!(
                (#call_function)(index, from_address, target_address, return_data, db_tx)
            )
        }
        (false, true, false) => {
            quote!(
                (#call_function)(index, from_address, target_address, log_data, db_tx)
            )
        }
        (false, false, false) => {
            quote!(
                (#call_function)(index, from_address, target_address, db_tx)
            )
        }
    };

    quote! {
        #[derive(Debug, Default)]
        pub struct #exchange_name;

        impl IntoAction for #exchange_name {
            fn get_signature(&self) -> [u8; 4] {
                #call_type::SELECTOR
            }

            #[allow(unused)]
            fn decode_trace_data(
                &self,
                index: u64,
                data: StaticReturnBindings,
                return_data: Bytes,
                from_address: Address,
                target_address: Address,
                logs: &Vec<Log>,
                db_tx: &LibmdbxTx<RO>,
            ) -> Option<Actions> {
                #(#option_parsing)*
                Some(Actions::#action_type(#fn_call?))
            }
        }
    }
    .into()
}

struct MacroParse {
    // required for all
    exchange_name: Ident,
    action_type:   Ident,
    log_type:      Ident,
    call_type:     Ident,

    /// for call data decoding
    exchange_mod_name: Ident,
    /// wether we want logs or not
    give_logs:         bool,
    /// wether we want return data or not
    give_returns:      bool,
    give_calldata:     bool,

    /// The closure that we use to construct the normalized type
    call_function: ExprClosure,
}

impl Parse for MacroParse {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let exchange_name: Ident = input.parse()?;
        input.parse::<Token![,]>()?;
        let action_type: Ident = input.parse()?;
        input.parse::<Token![,]>()?;
        let call_type: Ident = input.parse()?;
        input.parse::<Token![,]>()?;
        let log_type: Ident = input.parse()?;
        input.parse::<Token![,]>()?;

        let exchange_mod_name: Ident = input.parse()?;

        let mut logs = false;
        let mut return_data = false;
        let mut call_data = false;

        input.parse::<Token![,]>()?;

        while !input.peek(Token![|]) {
            let arg: Ident = input.parse()?;
            input.parse::<Token![:]>()?;
            let enabled: LitBool = input.parse()?;

            match arg.to_string().to_lowercase().as_str() {
                "logs" => logs = enabled.value(),
                "call_data" => call_data = enabled.value(),
                "return_data" => return_data = enabled.value(),
                _ => {
                    return Err(Error::new(
                        arg.span(),
                        format!(
                            "{} is not a valid config option, valid options are: \n logs , \
                             call_data, return_data",
                            arg,
                        ),
                    ))
                }
            }
            input.parse::<Token![,]>()?;
        }
        // no data enabled
        let call_function: ExprClosure = input.parse()?;

        if call_function.asyncness.is_some() {
            return Err(syn::Error::new(input.span(), "closure cannot be async"))
        }

        if !input.is_empty() {
            return Err(syn::Error::new(
                input.span(),
                "There should be no values after the call function",
            ))
        }

        if call_function.asyncness.is_some() {
            return Err(syn::Error::new(input.span(), "closure cannot be async"))
        }

        if !input.is_empty() {
            return Err(syn::Error::new(
                input.span(),
                "There should be no values after the call function",
            ))
        }

        Ok(Self {
            give_returns: return_data,
            log_type,
            call_function,
            give_logs: logs,
            give_calldata: call_data,
            call_type,
            action_type,
            exchange_name,
            exchange_mod_name,
        })
    }
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
/// for discovering new pools.
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
/// Curve is weird since each factory contract (7 of them) has multiple
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

#[proc_macro_derive(Transposable)]
pub fn transposable(item: TokenStream) -> TokenStream {
    let i_struct = parse_macro_input!(item as DeriveInput);
    transpose::parse(i_struct)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Simple utils for counters and gauges when it comes to tracking function
/// metrics, NOTE: tracks call once function has returned; early returns won't
/// be counted
#[proc_macro_attribute]
pub fn metrics_call(attr: TokenStream, item: TokenStream) -> TokenStream {
    let item = parse_macro_input!(item as ItemFn);
    function_metrics::parse(item, attr.into())
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
