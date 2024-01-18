use itertools::Itertools;
use proc_macro::{Span, TokenStream};
use quote::quote;
use syn::{bracketed, parse::Parse, Error, ExprClosure, Ident, Index, LitBool, Token};

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

    let mut is_possible_count = 0usize;
    let (log_idx, log_optional, log_field, log_ident): (
        Vec<Vec<Index>>,
        Vec<LitBool>,
        Vec<Ident>,
        Vec<Ident>,
    ) = log_types
        .into_iter()
        .enumerate()
        .filter_map(|(i, n)| {
            // is possible, need to increment count
            if n.0 {
                is_possible_count += 1;
            }
            if n.1 {
                return None
            }

            Some((
                (0..=is_possible_count)
                    .into_iter()
                    .map(|shift| Index::from(i - shift))
                    .collect_vec(),
                LitBool::new(n.0, Span::call_site().into()),
                Ident::new(&(n.2.to_string() + "_field"), Span::call_site().into()),
                n.2,
            ))
        })
        .multiunzip();

    let log_return_struct_name = Ident::new(
        &(exchange_name.to_string() + &action_type.to_string()),
        Span::call_site().into(),
    );
    let log_return_builder_struct_name = Ident::new(
        &(exchange_name.to_string() + &action_type.to_string() + "Builder"),
        Span::call_site().into(),
    );

    let res_struct_fields = log_optional
        .iter()
        .zip(log_ident.iter())
        .filter_map(|(optional, res)| {
            let field = Ident::new(&(res.to_string() + "_field"), Span::call_site().into());

            Some(if optional.value {
                quote!(#field : Option<crate::#exchange_mod_name::#res>)
            } else {
                quote!(#field : crate::#exchange_mod_name::#res)
            })
        })
        .collect_vec();

    let return_struct_build_fields = log_optional
        .iter()
        .zip(log_ident.iter())
        .filter_map(|(optional, res)| {
            let field = Ident::new(&(res.to_string() + "_field"), Span::call_site().into());

            Some(if optional.value {
                quote!(#field : self.#field)
            } else {
                quote!(#field : self.#field.unwrap())
            })
        })
        .collect_vec();

    let log_struct = if give_logs {
        quote!(
            struct #log_return_builder_struct_name {
                #(
                    #log_field: Option<crate::#exchange_mod_name::#log_ident>
                ),*
            }

            struct #log_return_struct_name {
                #(#res_struct_fields),*
            }

            impl #log_return_builder_struct_name {
                fn new() -> Self {
                    Self {
                        #(
                            #log_field: None
                        ),*
                    }
                }

                fn build(self) -> #log_return_struct_name {
                    #log_return_struct_name {
                        #(
                            #return_struct_build_fields
                        ),*
                    }
                }
            }
        )
    } else {
        quote!()
    };

    let a = call_type.to_string();
    let decalled = Ident::new(&a[..a.len() - 4], Span::call_site().into());

    if give_calldata {
        option_parsing.push(quote!(
                let call_data = crate::enum_unwrap!(data, #exchange_mod_name, #decalled);
        ));
    }

    if give_logs {
        option_parsing.push(quote!(
            let mut log_res = #log_return_builder_struct_name::new();
            #(
                'possible: {
                #(
                    if let Some(log)= &logs.get(#log_idx) {
                        if let Some(decoded)= <crate::#exchange_mod_name::#log_ident
                            as ::alloy_sol_types::SolEvent>
                            ::decode_log_data(&log.data, false).ok() {
                                log_res.#log_field = Some(decoded);
                               break 'possible 
                            }
                    }
                )*
                }
            )*
            let log_data = log_res.build();
        ));
    }

    if give_returns {
        option_parsing.push(quote!(
                let return_data = <crate::#exchange_mod_name::#call_type
                as alloy_sol_types::SolCall>
                ::abi_decode_returns(&return_data, false).map_err(|e| {
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
        #log_struct

        #[derive(Debug, Default)]
        pub struct #exchange_name;

        impl crate::IntoAction for #exchange_name {
            fn get_signature(&self) -> [u8; 4] {
                <#call_type as alloy_sol_types::SolCall>::SELECTOR
            }

            #[allow(unused)]
            fn decode_trace_data(
                &self,
                index: u64,
                data: crate::StaticReturnBindings,
                return_data: ::alloy_primitives::Bytes,
                from_address: ::alloy_primitives::Address,
                target_address: ::alloy_primitives::Address,
                logs: &Vec<::alloy_primitives::Log>,
                db_tx: &::brontes_database_libmdbx::implementation::tx::LibmdbxTx<
                ::reth_db::mdbx::RO
                >,
            ) -> Option<::brontes_types::normalized_actions::Actions> {
                #(#option_parsing)*
                Some(::brontes_types::normalized_actions::Actions::#action_type(#fn_call?))
            }
        }
    }
    .into()
}

struct MacroParse {
    // required for all
    exchange_name: Ident,
    action_type:   Ident,
    // (sometimes, ignore, ident)
    log_types:     Vec<(bool, bool, Ident)>,
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
            let mut possible = false;
            let mut ignore = false;

            if content.peek2(Token![<]) {
                let new_content = content.parse::<Ident>()?;
                if new_content.to_string().starts_with("Possible") {
                    possible = true;
                } else if new_content.to_string().starts_with("Ignore") {
                    ignore = true;
                } else {
                    return Err(syn::Error::new(
                        content.span(),
                        "Only valid modifiers are Possible and Ignore",
                    ))
                }
                let _ = content.parse::<Token![<]>()?;
            }

            // another modifier
            if content.peek2(Token![<]) {
                let new_content = content.parse::<Ident>()?;
                if new_content.to_string().starts_with("Possible") {
                    possible = true;
                } else if new_content.to_string().starts_with("Ignore") {
                    ignore = true;
                } else {
                    return Err(syn::Error::new(
                        content.span(),
                        "Only valid modifiers are Possible and Ignore",
                    ))
                }
                let _ = content.parse::<Token![<]>()?;
            }

            let Ok(log_type) = content.parse::<Ident>() else {
                break;
            };

            if content.peek(Token![>]) {
                let _ = content.parse::<Token![>]>()?;
            }
            if content.peek(Token![>]) {
                let _ = content.parse::<Token![>]>()?;
            }

            log_types.push((possible, ignore, log_type));

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


        impl crate::ActionCollection for #struct_name {

            fn dispatch(
                &self,
                sig: &[u8],
                index: u64,
                data: crate::StaticReturnBindings,
                return_data: ::alloy_primitives::Bytes,
                from_address: ::alloy_primitives::Address,
                target_address: ::alloy_primitives::Address,
                logs: &Vec<::alloy_primitives::Log>,
                db_tx: &::brontes_database_libmdbx::implementation::tx::LibmdbxTx<
                    ::reth_db::mdbx::RO
                >,
                block: u64,
                tx_idx: u64,
            ) -> Option<(
                    ::brontes_pricing::types::PoolUpdate,
                    ::brontes_types::normalized_actions::Actions
                )> {
                let hex_selector = ::alloy_primitives::Bytes::copy_from_slice(sig);

                if sig == crate::IntoAction::get_signature(&self.0) {
                    return crate::IntoAction::decode_trace_data(
                            &self.0,
                            index,
                            data,
                            return_data,
                            from_address,
                            target_address,
                            logs,
                            db_tx
                        ).map(|res| {
                        (::brontes_pricing::types::PoolUpdate {
                            block,
                            tx_idx,
                            logs: logs.clone(),
                            action: res.clone()
                        },
                        res)}).or_else(|| {
                            ::tracing::error!(
                                "classifier failed on function sig: {:?} for address: {:?}",
                                ::malachite::strings::ToLowerHexString::to_lower_hex_string(
                                    &hex_selector
                                ),
                                target_address.0,
                            );
                            None
                        })

                }
                #( else if sig == crate::IntoAction::get_signature(&self.#i) {
                     return crate::IntoAction::decode_trace_data(
                            &self.#i,
                            index,
                            data,
                            return_data,
                            from_address,
                            target_address,
                            logs,
                            db_tx
                    ).map(|res| {
                        (::brontes_pricing::types::PoolUpdate {
                            block,
                            tx_idx,
                            logs: logs.clone(),
                            action: res.clone()
                        },
                        res)}).or_else(|| {
                            ::tracing::error!(
                                "classifier failed on function sig: {:?} for address: {:?}",
                                ::malachite::strings::ToLowerHexString::to_lower_hex_string(
                                    &hex_selector
                                ),
                                target_address.0,
                            );
                            None
                        })

                    }
                )*

                ::tracing::debug!(
                    "no inspector for function selector: {:?} with contract address: {:?}",
                    ::malachite::strings::ToLowerHexString::to_lower_hex_string(
                        &hex_selector
                    ),
                    target_address.0,
                );

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
