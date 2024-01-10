use proc_macro::{Span, TokenStream};
use quote::quote;
use syn::{bracketed, parse::Parse, Error, ExprClosure, Ident, Index, LitBool, Token};

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
///     |index, from_address: Address, target_address: Address, log_data: (Swap)| { <body> });
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
pub fn action_impl(token_stream: TokenStream) -> TokenStream {
    let MacroParse {
        exchange_name,
        action_type,
        call_type,
        log_types,
        exchange_mod_name,
        give_logs,
        give_returns,
        call_function,
        give_calldata,
    } = syn::parse2(token_stream.into()).unwrap();

    let mut option_parsing = Vec::new();

    let (log_idx, log_type): (Vec<Index>, Vec<Ident>) = log_types
        .into_iter()
        .enumerate()
        .map(|(i, n)| (Index::from(i), n))
        .unzip();

    let a = call_type.to_string();
    let decalled = Ident::new(&a[..a.len() - 4], Span::call_site().into());

    if give_calldata {
        option_parsing.push(quote!(
                let call_data = enum_unwrap!(data, #exchange_mod_name, #decalled);
        ));
    }

    if give_logs {
        option_parsing.push(quote!(
            let log_data =
            (
                #(
                    {
                    let log = &logs[#log_idx];
                    #log_type::decode_log(log.topics.iter().map(|h|h.0), &log.data, false).ok()?
                    }

                )*
            );
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
            (#call_function)(
                index,
                from_address,
                target_address,
                call_data,
                return_data,
                log_data, db_tx
                )
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
    log_types:     Vec<Ident>,
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

        let mut log_types = Vec::new();

        let content;
        bracketed!(content in input);

        loop {
            let Ok(log_type) = content.parse::<Ident>() else {
                break;
            };
            log_types.push(log_type);

            let Ok(_) = content.parse::<Token![,]>() else {
                break;
            };
        }

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
            log_types,
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
pub fn action_dispatch(input: TokenStream) -> TokenStream {
    let ActionDispatch { struct_name, rest } = syn::parse2(input.into()).unwrap();

    if rest.is_empty() {
        panic!("need more than one entry");
    }

    let (mut i, name): (Vec<Index>, Vec<Ident>) = rest
        .into_iter()
        .enumerate()
        .map(|(i, n)| (Index::from(i), n))
        .unzip();
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
                db_tx: &LibmdbxTx<RO>,
                tx: UnboundedSender<::brontes_pricing::types::DexPriceMsg>,
                block: u64,
                tx_idx: u64,
            ) -> Option<Actions> {
                if sig == self.0.get_signature() {
                    let res = self.0.decode_trace_data(
                            index,
                            data,
                            return_data,
                            from_address,
                            target_address,
                            logs,
                            db_tx
                        );

                    if let Some(res) = &res {
                        let pool_update = PoolUpdate {
                            block,
                            tx_idx,
                            logs: logs.clone(),
                            action: res.clone()
                        };

                        tx.send(
                            ::brontes_pricing::types::DexPriceMsg::Update(pool_update)
                        ).unwrap();
                    }

                    return res
                }
                #( else if sig == self.#i.get_signature() {
                    let res = self.#i.decode_trace_data(
                            index,
                            data,
                            return_data,
                            from_address,
                            target_address,
                            logs,
                            db_tx
                    );
                        if let Some(res) = &res {
                            let pool_update = PoolUpdate {
                                logs: logs.clone(),
                                block,
                                tx_idx,
                                action: res.clone()
                            };

                            tx.send(
                                ::brontes_pricing::types::DexPriceMsg::Update(pool_update)
                            ).unwrap();

                        }
                            return res
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
    rest:        Vec<Ident>,
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
