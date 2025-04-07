use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{parse::Parse, Ident, Index, Token};

use super::ACTION_SIG_NAME;

pub struct ActionDispatch {
    // required for all
    struct_name: Ident,
    rest: Vec<Ident>,
}

impl ActionDispatch {
    pub fn expand(self) -> syn::Result<TokenStream> {
        let Self { struct_name, rest } = self;

        if rest.is_empty() {
            // Generate a compile_error! invocation as part of the output TokenStream
            return Err(syn::Error::new(Span::call_site(), "need classifiers to dispatch to"));
        }
        let (var_name, const_fns): (Vec<_>, Vec<_>) = rest
            .iter()
            .enumerate()
            .map(|(i, ident)| {
                (
                    Ident::new(&format!("VAR_{i}"), ident.span()),
                    Ident::new(&format!("{ACTION_SIG_NAME}_{}", ident), ident.span()),
                )
            })
            .unzip();

        let (i, name): (Vec<Index>, Vec<&Ident>) = rest
            .iter()
            .enumerate()
            .map(|(i, n)| (Index::from(i), n))
            .unzip();

        let match_stmt = expand_match_dispatch(&rest, &var_name, i);

        Ok(quote!(
                    #[derive(Default, Debug)]
                    pub struct #struct_name(#(pub #name,)*);

                    impl crate::ActionCollection for #struct_name {
                        fn dispatch<DB: ::brontes_database::libmdbx::LibmdbxReader
        + ::brontes_database::libmdbx::DBWriter
                            > (
                            &self,
                            call_info: ::brontes_types::structured_trace::CallFrameInfo<'_>,
                            db_tx: &DB,
                            block: u64,
                            tx_idx: u64,
                        ) -> Option<(
                                ::brontes_pricing::types::DexPriceMsg,
                                ::brontes_types::normalized_actions::Action
                            )> {


                            let protocol_byte = db_tx.get_protocol(call_info.target_address)
                                .ok()?.to_byte();

                            if call_info.call_data.len() < 4 {
                                return None
                            }

                            let hex_selector = ::alloy_primitives::Bytes::copy_from_slice(
                                &call_info.call_data[0..4]);

                            let sig = ::alloy_primitives::FixedBytes::<4>::from_slice(
                                &call_info.call_data[0..4]).0;

                            let mut sig_w_byte= [0u8; 5];
                            sig_w_byte[0..4].copy_from_slice(&sig);
                            sig_w_byte[4] = protocol_byte;


                            #(
                                const #var_name: [u8; 5] = #const_fns();
                            )*;

                            #match_stmt

                        }
                    }
                ))
    }
}

impl Parse for ActionDispatch {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let struct_name: Ident = input.parse()?;
        let mut rest = Vec::new();
        while input.parse::<Token![,]>().is_ok() {
            rest.push(input.parse::<Ident>()?);
        }

        if !input.is_empty() {
            return Err(syn::Error::new(input.span(), "Unwanted input at end of macro"));
        }

        Ok(Self { rest, struct_name })
    }
}

fn expand_match_dispatch(
    reg_name: &[Ident],
    var_name: &[Ident],
    var_idx: Vec<Index>,
) -> TokenStream {
    quote!(
        match sig_w_byte {
        #(
            #var_name => {
                let target_address = call_info.target_address;
                 return crate::IntoAction::decode_call_trace(
                    &self.#var_idx,
                    call_info,
                    block,
                    tx_idx,
                    db_tx
                ).map(|res| {
                    let action = res.get_action();
                    Some((res, action))
                 }).unwrap_or_else(|e| {
                        ::tracing::warn!(error=%e,
                            "classifier: {} failed on function sig: {:?} for address: {:?}",
                            stringify!(#reg_name),
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
            let target_address = call_info.target_address;
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
}
