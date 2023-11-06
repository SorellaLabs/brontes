use proc_macro::TokenStream;
use quote::quote;
use syn::{parenthesized, parse::Parse, token::Paren, ExprClosure, Ident, LitBool, Token};

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

    if !exchange_mod_name.to_string().eq("None") {
        has_calldata = true;
        option_parsing.push(quote!(
                let call_data = enum_unwrap!(data, #exchange_mod_name, #action_type);
        ));
    }

    if give_logs.value {
        option_parsing.push(quote!(
            let log_data = logs.into_iter().filter_map(|log| {
                #action_type::decode_log(log.topics.iter().map(|h| h.0), &log.data, true).ok()
            }).collect::<Vec<_>>();
            let log_data = Some(log_data).filter(|data| !data.is_empty()).map(|mut l| l.remove(0));
        ));
    }

    //println!("tt is not the name 0");
    if give_returns.value {
        option_parsing.push(quote!(
                let return_data = #call_type::abi_decode_returns(&return_data, true).unwrap();
        ));
    }
    // println!("tt is the name 0");

    let fn_call = match (has_calldata, give_logs.value, give_returns.value) {
        (true, true, true) => {
            quote!(
            (#call_function)(index, from_address, target_address, call_data, return_data, log_data)
            )
        }
        (true, true, false) => {
            quote!(
                (#call_function)(index, from_address, target_address, call_data, log_data)
            )
        }
        (true, false, true) => {
            quote!(
                (#call_function)(index, from_address, target_address, call_data, return_data)
            )
        }
        (true, false, false) => {
            quote!(
                (#call_function)(index, from_address, target_address, call_data)
            )
        }
        (false, true, true) => {
            quote!(
                (#call_function)(index, from_address, target_address, return_data, log_data)
            )
        }
        (false, false, true) => {
            quote!(
                (#call_function)(index, from_address, target_address, return_data)
            )
        }
        (false, true, false) => {
            quote!(
                (#call_function)(index, from_address, target_address, log_data)
            )
        }
        (false, false, false) => {
            quote!(
                (#call_function)(index, from_address, target_address)
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
    action_type: Ident,
    call_type: Ident,

    /// needed if we decide to decode call data
    exchange_mod_name: Ident,
    /// wether we want logs or not
    give_logs: LitBool,
    /// wether we want return data or not
    give_returns: LitBool,

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
        let mut exchange_mod_name: Ident = input.parse()?;

        if input.peek(Paren) {
            let content;
            parenthesized!(content in input);
            exchange_mod_name = content.parse()?;
        }

        input.parse::<Token![,]>()?;
        let give_logs: LitBool = input.parse()?;
        input.parse::<Token![,]>()?;
        let give_returns: LitBool = input.parse()?;
        input.parse::<Token![,]>()?;
        let call_function: ExprClosure = input.parse()?;

        if call_function.asyncness.is_some() {
            return Err(syn::Error::new(input.span(), "closure cannot be async"));
        }

        if !input.is_empty() {
            return Err(syn::Error::new(
                input.span(),
                "There should be no values after the call function",
            ));
        }

        Ok(Self {
            give_returns,
            call_function,
            give_logs,
            call_type,
            action_type,
            exchange_name,
            exchange_mod_name,
        })
    }
}

#[proc_macro]
pub fn action_dispatch(input: TokenStream) -> TokenStream {
    let ActionDispatch { struct_name, rest } = syn::parse2(input.into()).unwrap();

    if rest.is_empty() {
        panic!("need more than one entry");
    }

    let (mut i, name): (Vec<usize>, Vec<Ident>) = rest.into_iter().enumerate().unzip();
    i.remove(0);

    quote!(
        #[derive(Default, Debug)]
        pub struct #struct_name(#(pub #name,)*);

        impl ActionCollection for #struct_name {
            fn dispatch(
                &self,
                sig: &[u8],
                index: u64,
                data: StaticReturnBindings,
                return_data: Bytes,
                from_address: Address,
                target_address: Address,
                logs: &Vec<Log>,
            ) -> Option<Actions> {
                if sig == self.0.get_signature() {
                    return
                        self.0.decode_trace_data(
                            index,
                            data,
                            return_data,
                            from_address,
                            target_address,
                            logs,
                            )
                }

                #( else if sig == self.#i.get_signature() {
                        return self.#i.decode_trace_data(
                            index,
                            data,
                            return_data,
                            from_address,
                            target_address,
                            logs,
                        )
                    }
                )*

                None
            }
        }
    )
    .into()
}

struct ActionDispatch {
    // required for all
    struct_name: Ident,
    rest: Vec<Ident>,
}
impl Parse for ActionDispatch {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let struct_name: Ident = input.parse()?;
        let mut rest = Vec::new();
        while input.parse::<Token![,]>().is_ok() {
            rest.push(input.parse::<Ident>()?);
        }
        if !input.is_empty() {
            panic!("unkown characters")
        }

        Ok(Self { rest, struct_name })
    }
}
