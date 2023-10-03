use proc_macro::TokenStream;
use quote::quote;
use syn::{parse::Parse, ExprClosure, Ident, LitBool, Token};

#[proc_macro]
/// the action impl macro deals with automatically parsing the data needed for
/// underlying actions. The use is as followed
/// ```rust
/// action_impl!(ExchagneName, ActionDecodeType, ActionCallType, Option<ExchangeModName>, GiveLogs, GiveReturns, CallFn)
/// ```
/// Where GiveLogs, GiveReturns are bools, and CallFn is a closure that takes
/// ```rust
/// |index, from_address, target_address, call_data, return_data, log_data| { <body> }
/// ```
pub fn action_impl(token_stream: TokenStream) -> TokenStream {
    let MacroParse {
        exchange_name,
        action_type,
        call_type,
        exchange_mod_name,
        give_logs,
        give_returns,
        call_function,
    } = syn::parse2(token_stream.into()).unwrap();

    let mut has_calldata = false;
    let mut option_parsing = Vec::new();

    if exchange_mod_name.starts_with("Some") {
        has_calldata = true;
        let module_name = &exchange_mod_name[4..exchange_mod_name.len() - 1];
        option_parsing.push(quote!(
                let call_data = enum_unwrap!(data, #module_name, #call_type);
        ));
    }

    if give_logs.value {
        option_parsing.push(quote!(
            let log_data = logs.into_iter().filter_map(|log| {
                #action_type::decode_log(log.topics.iter().map(|h| h.0), &log.data, true).ok()
            }).collect::<Vec<_>>().remove(0);
        ));
    }

    if give_returns.value {
        option_parsing.push(quote!(
                let return_data = #call_type::abi_decode_returns(&return_data, true).unwrap();
        ));
    }

    let fn_call = match (has_calldata, give_logs.value, give_returns.value) {
        (true, true, true) => {
            quote!(
            #call_function(index, from_address, target_address, call_data, return_data, log_data)
            )
        }
        (true, true, false) => {
            quote!(
                #call_function(index, from_address, target_address, call_data, log_data)
            )
        }
        (true, false, true) => {
            quote!(
                #call_function(index, from_address, target_address, call_data, return_data)
            )
        }
        (true, false, false) => {
            quote!(
                #call_function(index, from_address, target_address, call_data)
            )
        }
        (false, true, true) => {
            quote!(
                #call_function(index, from_address, target_address, return_data, log_data)
            )
        }
        (false, false, true) => {
            quote!(
                #call_function(index, from_address, target_address, return_data)
            )
        }
        (false, true, false) => {
            quote!(
                #call_function(index, from_address, target_address, log_data)
            )
        }
        (false, false, false) => {
            quote!(
                #call_function(index, from_address, target_address)
            )
        }
    };

    quote! {
        #[derive(Debug, Default)]
        pub struct #exchange_name;

        impl IntoAction for #exchange_name {
            fn get_signature(&self) -> [u8; 4] {
                $call_type::SELECTOR
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
            ) -> Actions {
                #(#option_parsing)*
                Actions::#action_type(#fn_call)
            }
        }
    }
    .into()
}

struct MacroParse {
    // required for all
    exchange_name: Ident,
    action_type:   Ident,
    call_type:     Ident,

    /// needed if we decide to decode call data
    exchange_mod_name: String,
    /// wether we want logs or not
    give_logs:         LitBool,
    /// wether we want return data or not
    give_returns:      LitBool,

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
        let exchange_mod_name: Ident = input.parse()?;
        input.parse::<Token![,]>()?;
        let give_logs: LitBool = input.parse()?;
        input.parse::<Token![,]>()?;
        let give_returns: LitBool = input.parse()?;
        input.parse::<Token![,]>()?;
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

        Ok(Self {
            give_returns,
            call_function,
            give_logs,
            call_type,
            action_type,
            exchange_name,
            exchange_mod_name: exchange_mod_name.to_string(),
        })
    }
}
