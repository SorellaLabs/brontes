use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{
    bracketed,
    parse::{Parse, ParseBuffer},
    spanned::Spanned,
    Error, ExprClosure, Ident, LitBool, Path, Token,
};

use super::{data_preparation::CallDataParsing, ACTION_SIG_NAME};

pub struct ActionMacro {
    // required for all
    protocol_path:        Path,
    path_to_call:         Path,
    action_type:          Ident,
    exchange_name_w_call: Ident,
    log_types:            Vec<(bool, bool, Ident)>,
    /// wether we want logs or not
    give_logs:            bool,
    /// wether we want return data or not
    give_returns:         bool,
    /// wether we want call_data or not
    give_call_data:       bool,
    /// The closure that we use to construct the normalized type
    call_function:        ExprClosure,
}

impl ActionMacro {
    pub fn expand(self) -> syn::Result<TokenStream> {
        let Self {
            exchange_name_w_call,
            protocol_path,
            action_type,
            path_to_call,
            log_types,
            give_logs,
            give_call_data,
            give_returns,
            call_function,
        } = self;

        let call_data = CallDataParsing::new(
            give_logs,
            give_call_data,
            give_returns,
            &exchange_name_w_call,
            &action_type,
            &path_to_call,
            &log_types,
            call_function,
        );

        let call_fn_name =
            Ident::new(&format!("{ACTION_SIG_NAME}_{}", exchange_name_w_call), Span::call_site());

        Ok(quote! {
            #[allow(unused_imports)]
            use #path_to_call;

            #[allow(non_snake_case)]
            pub const fn #call_fn_name() -> [u8; 5] {
                ::alloy_primitives::FixedBytes::new(
                        <#path_to_call as ::alloy_sol_types::SolCall>::SELECTOR
                    )
                    .concat_const(
                    ::alloy_primitives::FixedBytes::new(
                        [#protocol_path.to_byte()]
                        )
                    ).0
            }

            #[derive(Debug, Default)]
            pub struct #exchange_name_w_call;

            impl crate::IntoAction for #exchange_name_w_call {
                fn decode_trace_data<DB: ::brontes_database::libmdbx::LibmdbxReader>(
                    &self,
                    index: u64,
                    data: ::alloy_primitives::Bytes,
                    return_data: ::alloy_primitives::Bytes,
                    from_address: ::alloy_primitives::Address,
                    target_address: ::alloy_primitives::Address,
                    msg_sender: ::alloy_primitives::Address,
                    logs: &Vec<::alloy_primitives::Log>,
                    db_tx: &DB
                    ) -> Option<::brontes_types::normalized_actions::Actions> {
                    #call_data
                    Some(::brontes_types::normalized_actions::Actions::#action_type(result))
                }
            }
        })
    }
}

impl Parse for ActionMacro {
    fn parse(mut input: syn::parse::ParseStream) -> syn::Result<Self> {
        let protocol_path = parse_protocol_path(&mut input)?;
        input.parse::<Token![,]>()?;

        let path_to_call = parse_decode_fn_path(&mut input)?;
        input.parse::<Token![,]>()?;

        let action_type: Ident = input.parse()?;
        input.parse::<Token![,]>()?;

        let possible_logs = parse_logs(&mut input)?;
        input.parse::<Token![,]>()?;

        let (logs, return_data, call_data) = parse_config(&mut input)?;
        let call_function = parse_closure(&mut input)?;

        let exchange_name_w_call = Ident::new(
            &format!(
                "{}{}",
                protocol_path.segments[protocol_path.segments.len() - 1].ident,
                path_to_call.segments[path_to_call.segments.len() - 1].ident
            ),
            Span::call_site(),
        );

        Ok(Self {
            path_to_call,
            give_returns: return_data,
            log_types: possible_logs,
            call_function,
            give_logs: logs,
            give_call_data: call_data,
            action_type,
            protocol_path,
            exchange_name_w_call,
        })
    }
}

fn parse_closure(input: &mut syn::parse::ParseStream) -> syn::Result<ExprClosure> {
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

    Ok(call_function)
}

fn parse_config(input: &mut syn::parse::ParseStream) -> syn::Result<(bool, bool, bool)> {
    let mut logs = false;
    let mut return_data = false;
    let mut call_data = false;

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
                        "{} is not a valid config option, valid options are: \n logs , call_data, \
                         return_data",
                        arg,
                    ),
                ))
            }
        }
        input.parse::<Token![,]>()?;
    }

    Ok((logs, return_data, call_data))
}

fn parse_protocol_path(input: &mut syn::parse::ParseStream) -> syn::Result<Path> {
    let protocol_path: Path = input.parse().map_err(|_| {
        syn::Error::new(input.span(), "No Protocol Found, Should be Protocol::<ProtocolVarient>")
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
    Ok(protocol_path)
}

fn parse_decode_fn_path(input: &mut syn::parse::ParseStream) -> syn::Result<Path> {
    let fn_path: Path = input.parse().map_err(|_| {
        syn::Error::new(
            input.span(),
            "No path to alloy fn found, Should be path::to::alloy::call::fn_nameCall",
        )
    })?;

    if fn_path.segments.len() < 2 {
        return Err(syn::Error::new(
            fn_path.span(),
            "incorrect path, Should be <crate>::<path_to>::ProtocolModName::FnCall",
        ))
    }

    Ok(fn_path)
}

fn parse_logs(input: &mut syn::parse::ParseStream) -> syn::Result<Vec<(bool, bool, Ident)>> {
    let mut log_types = Vec::new();
    let mut content;
    bracketed!(content in input);

    loop {
        let mut possible = false;
        let mut ignore = false;

        if content.peek2(Token![<]) {
            parse_log_modifier(&mut content, &mut possible, &mut ignore)?;
        }
        if content.peek2(Token![<]) {
            parse_log_modifier(&mut content, &mut possible, &mut ignore)?;
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

    Ok(log_types)
}

fn parse_log_modifier(
    content: &mut ParseBuffer,
    possible: &mut bool,
    ignore: &mut bool,
) -> syn::Result<()> {
    let new_content = content.parse::<Ident>()?;
    if new_content.to_string().starts_with("Possible") {
        *possible = true;
    } else if new_content.to_string().starts_with("Ignore") {
        *ignore = true;
    } else {
        return Err(syn::Error::new(content.span(), "Only valid modifiers are Possible and Ignore"))
    }
    let _ = content.parse::<Token![<]>()?;
    Ok(())
}
