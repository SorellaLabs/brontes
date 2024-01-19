use proc_macro::TokenStream;
use quote::quote;
use syn::{parse::Parse, ExprClosure, Ident, Index, LitBool, Token};

pub fn discovery_impl(token_stream: TokenStream) -> TokenStream {
    let MacroParse {
        decoder_name,
        factory_name,
        event_type,
        has_token_fields,
        needs_reth_handle,
        address_call_function,
    } = syn::parse2(token_stream.into()).unwrap();

    let mut option_parsing = Vec::new();

    if needs_reth_handle.value {
        option_parsing.push(quote!(
            let decoded_events_handle = logs.into_iter().filter_map(|log| {
                let handle = node_handle.clone();
                let pool_addr = Box::pin(async move {
                    let Some(transfer_log) =
                        get_log_from_tx(
                            handle,
                            block_number,
                            tx_hash,
                            crate::#factory_name::#event_type::SIGNATURE_HASH,
                            2).await
                    else {
                        return None;
                    };

                    let Some(decoded_transfer_log) = <crate::Transfer
                        as ::alloy_sol_types::SolEvent>
                        ::decode_log(&transfer_log, true).ok() else {
                        // log!(RawEthNewPoolsResults, 1, "Error Decoding", protocol, "Inner ::alloy_primitives::Log For Address", transfer_log);
                        return None;
                    };

                    Some(decoded_transfer_log.to)
                }) as ::std::pin::Pin<Box<dyn ::futures::Future<Output =
                Option<::alloy_primitives::Address>> + Send>>;

                let Some(val) = <#factory_name::#event_type as ::alloy_sol_types::SolEvent>
                    ::decode_log(&log, true).ok() else {
                    // log!(RawEthNewPoolsResults, 1, "Error Decoding", protocol, "::alloy_primitives::Log", log);
                    return None;
                };

                Some((val, block_number, pool_addr))
            }).collect::<Vec<_>>();
        ));
    } else {
        option_parsing.push(quote!(
            let decoded_events = logs.into_iter().filter_map(|log| {
                let val = <crate::#factory_name::#event_type as ::alloy_sol_types::SolEvent>
                    ::decode_log(&log, true).ok();
                if val.is_none() {
                    // log!(RawEthNewPoolsResults, 1, "Error Decoding", protocol, "::alloy_primitives::Log", log);
                }
                val.map(|v| (v, block_number))
            }).collect::<Vec<_>>();
        ));
    }

    let fn_call = match (has_token_fields.value, needs_reth_handle.value) {
        (true, false) => {
            quote!(
                async move {(#address_call_function)(protocol, decoded_events)}
            )
        }
        (false, true) => {
            quote!(
                (#address_call_function)(node_handle.clone(), protocol, decoded_events_handle)
            )
        }
        _ => unreachable!("Can't do this"),
    };

    quote! {
        #[derive(Debug, Default)]
        pub struct #decoder_name;

        impl crate::FactoryDecoder for #decoder_name {
            fn get_signature(&self) -> [u8; 32] {
                <crate::#factory_name::#event_type as ::alloy_sol_types::SolEvent>::SIGNATURE_HASH.0
            }


            #[allow(unused)]
            async fn decode_new_pool<T: ::brontes_types::traits::TracingProvider> (
                &self,
                node_handle: ::std::sync::Arc<T>,
                protocol: ::brontes_types::exchanges::StaticBindingsDb,
                logs: &Vec<::alloy_primitives::Log>,
                block_number: u64,
                tx_hash: B256,
            ) -> Vec<::brontes_pricing::types::DiscoveredPool> {
                #(#option_parsing)*
                #fn_call.await
            }
        }
    }
    .into()
}

struct MacroParse {
    // required for all
    decoder_name: Ident,
    factory_name: Ident,
    event_type:   Ident,

    /// if the tokens are taken from the decoded fields
    has_token_fields:  LitBool,
    /// if the reth handle is needed to get the tokens
    needs_reth_handle: LitBool,

    /// The closure that we use to get the address of the pool
    address_call_function: ExprClosure,
}

impl Parse for MacroParse {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let decoder_name: Ident = input.parse()?;
        input.parse::<Token![,]>()?;
        let factory_name: Ident = input.parse()?;
        input.parse::<Token![,]>()?;
        let event_type: Ident = input.parse()?;
        input.parse::<Token![,]>()?;
        let has_token_fields: LitBool = input.parse()?;
        input.parse::<Token![,]>()?;
        let needs_reth_handle: LitBool = input.parse()?;
        input.parse::<Token![,]>()?;
        let address_call_function: ExprClosure = input.parse()?;

        if !input.is_empty() {
            return Err(syn::Error::new(
                input.span(),
                "There should be no values after the call function",
            ))
        }

        Ok(Self {
            decoder_name,
            factory_name,
            event_type,
            has_token_fields,
            needs_reth_handle,
            address_call_function,
        })
    }
}

pub fn discovery_dispatch(input: TokenStream) -> TokenStream {
    let ActionDispatch { struct_name, rest } = syn::parse2(input.into()).unwrap();

    let (mut i, name): (Vec<Index>, Vec<Ident>) = rest
        .into_iter()
        .enumerate()
        .map(|(i, n)| (Index::from(i), n))
        .unzip();
    i.remove(0);

    quote!(
        #[derive(Default, Debug)]
        pub struct #struct_name(#(pub #name,)*);

        impl crate::FactoryDecoderDispatch for #struct_name {
            async fn dispatch<T: ::brontes_types::traits::TracingProvider>(
                sig: [u8; 32],
                node_handle: ::std::sync::Arc<T>,
                protocol: ::brontes_types::exchanges::StaticBindingsDb,
                logs: &Vec<::alloy_primitives::Log>,
                block_number: u64,
                tx_hash: B256,
                ) -> Vec<::brontes_pricing::types::DiscoveredPool> {
                let this = Self::default();
                if sig == crate::FactoryDecoder::get_signature(&this.0) {
                    return
                        crate::FactoryDecoder::decode_new_pool(
                            &this.0,
                            node_handle,
                            protocol,
                            logs,
                            block_number,
                            tx_hash,
                        ).await
                }
                #( else if sig == crate::FactoryDecoder::get_signature(&this.#i) {
                        return crate::FactoryDecoder::decode_new_pool(
                            &this.#i,
                            node_handle,
                            protocol,
                            logs,
                            block_number,
                            tx_hash,

                        ).await
                    }
                )*
                Vec::new()
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
