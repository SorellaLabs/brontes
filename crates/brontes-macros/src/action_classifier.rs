use itertools::Itertools;
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{
    bracketed, parse::Parse, spanned::Spanned, Error, ExprClosure, Ident, Index, LitBool, Path,
    Token,
};

//TODO: Remove need for writing out args that are always passed in the closure
// like: from_address, target_address, index
// Allow for passing a default config struct for optional args

pub fn action_impl(token_stream: TokenStream) -> syn::Result<TokenStream> {
    let MacroParse {
        protocol_path,
        action_type,
        call_type,
        log_types,
        exchange_mod_name,
        give_logs,
        give_returns,
        call_function,
        give_calldata,
    } = syn::parse2(token_stream)?;

    let exchange_name = Ident::new(
        &format!("{}{}", protocol_path.segments[protocol_path.segments.len() - 1].ident, call_type),
        Span::call_site(),
    );

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
        // collect to set all indexes
        .collect_vec()
        .into_iter()
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
                    .filter_map(|shift| {
                        if i < shift {
                            return None
                        }
                        Some(Index::from(i - shift))
                    })
                    .collect_vec(),
                LitBool::new(n.0, Span::call_site()),
                Ident::new(&(n.2.to_string() + "_field"), Span::call_site()),
                n.2,
            ))
        })
        .multiunzip();

    let log_return_struct_name =
        Ident::new(&(exchange_name.to_string() + &action_type.to_string()), Span::call_site());

    let log_return_builder_struct_name = Ident::new(
        &(exchange_name.to_string() + &action_type.to_string() + "Builder"),
        Span::call_site(),
    );

    let res_struct_fields = log_optional
        .iter()
        .zip(log_ident.iter())
        .filter_map(|(optional, res)| {
            let field = Ident::new(&(res.to_string() + "_field"), Span::call_site());

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
            let field = Ident::new(&(res.to_string() + "_field"), Span::call_site());

            Some(if optional.value {
                // don't unwrap optional
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

    if give_calldata {
        option_parsing.push(quote!(
            let call_data = <crate::#exchange_mod_name::#call_type
                as ::alloy_sol_types::SolCall>::abi_decode(&data[..],false).ok()?;
        ));
    }

    if give_logs {
        option_parsing.push(quote!(
            let mut log_res = #log_return_builder_struct_name::new();
            #(
                'possible: {
                #(
                    if let Some(log) = &logs.get(#log_idx) {
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
                msg_sender,
                call_data,
                return_data,
                log_data,
                db_tx
                )
            )
        }
        (true, true, false) => {
            quote!(
                (#call_function)(
                    index,
                    from_address,
                    target_address,
                    msg_sender,
                    call_data,
                    log_data,
                    db_tx
                    )
            )
        }
        (true, false, true) => {
            quote!(
                (#call_function)(
                    index,
                    from_address,
                    target_address,
                    msg_sender,
                    call_data,
                    return_data,
                    db_tx
                )
            )
        }
        (true, false, false) => {
            quote!(
                (#call_function)
                (index,
                 from_address,
                 target_address,
                 msg_sender,
                 call_data,
                 db_tx)
            )
        }
        (false, true, true) => {
            quote!(
                (#call_function)(
                    index,
                    from_address,
                    target_address,
                    msg_sender,
                    return_data,
                    log_data,
                    db_tx)
            )
        }
        (false, false, true) => {
            quote!(
                (#call_function)(
                    index,
                    from_address,
                    target_address,
                    msg_sender,
                    return_data,
                    db_tx
                )
            )
        }
        (false, true, false) => {
            quote!(
                (#call_function)(
                    index,
                    from_address,
                    target_address,
                    msg_sender,
                    log_data,
                    db_tx)
            )
        }
        (false, false, false) => {
            quote!(
                (#call_function)(index, from_address, target_address, msg_sender, db_tx)
            )
        }
    };

    let call_fn_name = Ident::new(&format!("__{}_action_sig", exchange_name), Span::call_site());

    Ok(quote! {
        #log_struct

        #[allow(non_snake_case)]
        pub const fn #call_fn_name() -> [u8; 5] {
            ::alloy_primitives::FixedBytes::new(
                    <crate::#exchange_mod_name::#call_type as ::alloy_sol_types::SolCall>::SELECTOR
                )
                .concat_const(
                ::alloy_primitives::FixedBytes::new(
                    [#protocol_path.to_byte()]
                    )
                ).0
        }

        #[derive(Debug, Default)]
        pub struct #exchange_name;

        impl crate::IntoAction for #exchange_name {
            fn decode_trace_data(
                &self,
                index: u64,
                data: ::alloy_primitives::Bytes,
                return_data: ::alloy_primitives::Bytes,
                from_address: ::alloy_primitives::Address,
                target_address: ::alloy_primitives::Address,
                msg_sender: ::alloy_primitives::Address,
                logs: &Vec<::alloy_primitives::Log>,
                db_tx: &brontes_database::libmdbx::tx::CompressedLibmdbxTx<
                    ::reth_db::mdbx::RO
                >,
            ) -> Option<::brontes_types::normalized_actions::Actions> {
                #(#option_parsing)*
                Some(::brontes_types::normalized_actions::Actions::#action_type(#fn_call?))
            }
        }
    })
}

struct MacroParse {
    // required for all
    protocol_path: Path,
    action_type:   Ident,
    // (sometimes, ignore, ident)
    // TODO: could make optional
    log_types:     Vec<(bool, bool, Ident)>,
    call_type:     Ident,

    /// alloy sol! generated mod for call data decoding
    //TODO: better name
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
        let protocol_path: Path = input.parse().map_err(|_| {
            syn::Error::new(
                input.span(),
                "No Protocol Found, Should be Protocol::<ProtocolVarient>",
            )
        })?;

        if protocol_path.segments.len() < 2 {
            return Err(syn::Error::new(
                protocol_path.span(),
                "incorrect path, Should be Protocol::<ProtocolVarient>",
            ))
        }

        let should_protocol = &protocol_path.segments[protocol_path.segments.len() - 2].ident;
        if !should_protocol.to_string().starts_with("Protocol") {
            return Err(syn::Error::new(
                should_protocol.span(),
                "incorrect path, Should be Protocol::<ProtocolVarient>",
            ))
        }

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
            protocol_path,
            exchange_mod_name,
        })
    }
}

pub fn action_dispatch(input: TokenStream) -> syn::Result<TokenStream> {
    let ActionDispatch { struct_name, rest } = syn::parse2(input.into())?;

    if rest.is_empty() {
        // Generate a compile_error! invocation as part of the output TokenStream
        return Err(syn::Error::new(Span::call_site(), "need classifiers to dispatch to"))
    }
    let (var_name, const_fns): (Vec<_>, Vec<_>) = rest
        .iter()
        .enumerate()
        .map(|(i, ident)| {
            (
                Ident::new(&format!("VAR_{i}"), ident.span()),
                Ident::new(&format!("__{}_action_sig", ident), ident.span()),
            )
        })
        .unzip();

    let (i, name): (Vec<Index>, Vec<Ident>) = rest
        .into_iter()
        .enumerate()
        .map(|(i, n)| (Index::from(i), n))
        .unzip();

    Ok(quote!(
        #[derive(Default, Debug)]
        pub struct #struct_name(#(pub #name,)*);


        impl crate::ActionCollection for #struct_name {
            fn dispatch(
                &self,
                index: u64,
                data: ::alloy_primitives::Bytes,
                return_data: ::alloy_primitives::Bytes,
                from_address: ::alloy_primitives::Address,
                target_address: ::alloy_primitives::Address,
                msg_sender: ::alloy_primitives::Address,
                logs: &Vec<::alloy_primitives::Log>,
                db_tx: &brontes_database::libmdbx::tx::CompressedLibmdbxTx<
                    ::reth_db::mdbx::RO
                >,
                block: u64,
                tx_idx: u64,
            ) -> Option<(
                    ::brontes_pricing::types::PoolUpdate,
                    ::brontes_types::normalized_actions::Actions
                )> {


                let hex_selector = ::alloy_primitives::Bytes::copy_from_slice(&data[0..4]);

                let sig = ::alloy_primitives::FixedBytes::<4>::from_slice(&data[0..4]).0;
                let protocol_byte = db_tx.get::<
                    ::brontes_database::libmdbx::tables::AddressToProtocol>
                    (target_address).ok()??.to_byte();

                let mut sig_w_byte= [0u8;5];
                sig_w_byte[0..4].copy_from_slice(&sig);
                sig_w_byte[4] = protocol_byte;


                #(
                    const #var_name: [u8; 5] = #const_fns();
                )*;

                match sig_w_byte {
                #(
                    #var_name => {
                         return crate::IntoAction::decode_trace_data(
                                &self.#i,
                                index,
                                data,
                                return_data,
                                from_address,
                                target_address,
                                msg_sender,
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

                    _ => {
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

            }
        }
    ))
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
            return Err(syn::Error::new(input.span(), "Unkown imput"))
        }

        Ok(Self { rest, struct_name })
    }
}
