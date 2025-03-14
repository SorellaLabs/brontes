use proc_macro2::{Literal, Span, TokenStream};
use quote::quote;
use syn::{parse::Parse, ExprClosure, Ident, Index, Path, Token};
pub mod curve;

pub fn discovery_impl(token_stream: TokenStream) -> syn::Result<TokenStream> {
    let MacroParse { discovery_name, function_call_path, factory_address, address_call_function } =
        syn::parse2(token_stream)?;

    is_proper_address(&factory_address)?;
    let stripped_address = &factory_address.to_string()[2..];
    let discovery_name_str = discovery_name.to_string();
    let mod_name = Ident::new(&format!("{}_mod", discovery_name_str), discovery_name.span());

    let fn_name = Ident::new(&format!("__{}_address_and_fn", discovery_name), Span::call_site());

    Ok(quote! (
        pub use #mod_name::#discovery_name;
        pub use #mod_name::#fn_name;

        #[allow(non_snake_case)]
        mod #mod_name {
            use #function_call_path;
            use super::*;
            use ::brontes_types::normalized_actions::pool::NormalizedNewPool;

            pub const fn #fn_name() -> [u8; 24] {
                    ::alloy_primitives::FixedBytes::new(::alloy_primitives::hex!(#stripped_address))
                        .concat_const(
                        ::alloy_primitives::FixedBytes::new(
                            <#function_call_path as ::alloy_sol_types::SolCall>::SELECTOR
                            )
                        ).0
            }

            #[derive(Debug, Default)]
            pub struct #discovery_name;

            impl crate::FactoryDiscovery for #discovery_name {

                async fn decode_create_trace<T: ::brontes_types::traits::TracingProvider>(
                    &self,
                    tracer: ::std::sync::Arc<T>,
                    deployed_address: ::alloy_primitives::Address,
                    trace_idx: u64,
                    parent_calldata: ::alloy_primitives::Bytes,
                ) -> Vec<::brontes_types::normalized_actions::pool::NormalizedNewPool>{
                    let Ok(decoded_data) = <#function_call_path
                        as ::alloy_sol_types::SolCall>::abi_decode(&parent_calldata[..], false)
                        else {
                            ::tracing::error!(target: "brontes_classifier::discovery", "{} failed to decode calldata", #discovery_name_str);
                            return Vec::new();
                    };
                    let res = (#address_call_function)
                        (deployed_address, trace_idx, decoded_data, tracer)
                        .await;
                    if res.is_empty() {
                            ::tracing::error!(target: "brontes_classifier::discovery", "discovery classifier returned nothing");
                    }

                    res
                }
            }
        }
    ))
}

fn is_proper_address(possible_address: &Literal) -> syn::Result<()> {
    let stred = possible_address.to_string();
    if !stred.starts_with("0x") {
        return Err(syn::Error::new(
            possible_address.span(),
            "Supplied factory address is invalid. Needs to start with 0x",
        ));
    }
    if stred.len() != 42 {
        return Err(syn::Error::new(
            possible_address.span(),
            format!("Supplied factory address length is incorrect got: {} wanted: 40", stred.len()),
        ));
    }

    Ok(())
}

struct MacroParse {
    // required for all
    discovery_name:        Ident,
    function_call_path:    Path,
    factory_address:       Literal,
    /// The closure that we use to get the address of the pool
    address_call_function: ExprClosure,
}

impl Parse for MacroParse {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let discovery_name: Ident = input
            .parse()
            .map_err(|e| syn::Error::new(e.span(), "Failed to parse discovery name"))?;
        input.parse::<Token![,]>()?;
        let function_call_path: Path = input
            .parse()
            .map_err(|e| syn::Error::new(e.span(), "Failed to parse path to function call"))?;
        input.parse::<Token![,]>()?;
        let factory_address: Literal = input.parse()?;
        input.parse::<Token![,]>()?;
        let address_call_function: ExprClosure = input.parse()?;

        if !input.is_empty() {
            return Err(syn::Error::new(
                input.span(),
                "There should be no values after the call function",
            ));
        }

        Ok(Self { discovery_name, factory_address, function_call_path, address_call_function })
    }
}

pub fn discovery_dispatch(input: TokenStream) -> syn::Result<TokenStream> {
    let DiscoveryDispatch { struct_name, rest } = syn::parse2(input)?;

    let (var_name, fn_name): (Vec<_>, Vec<_>) = rest
        .iter()
        .enumerate()
        .map(|(i, n)| {
            (
                Ident::new(&format!("VAR_{i}"), n.span()),
                Ident::new(&format!("__{}_address_and_fn", n), n.span()),
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

        impl crate::FactoryDiscoveryDispatch for #struct_name {
            async fn dispatch<T: ::brontes_types::traits::TracingProvider>(
                    &self,
                    tracer: ::std::sync::Arc<T>,
                    search_data: ::std::vec::Vec<(::alloy_primitives::Address,
                        ::alloy_primitives::Bytes)>,
                    deployed_address: ::alloy_primitives::Address,
                    trace_idx: u64,
                ) ->Vec<::brontes_types::normalized_actions::pool::NormalizedNewPool> {

                    ::futures::stream::iter(search_data)
                        .map(|(factory, parent_calldata)| {
                            let tracer = tracer.clone();
                            async move {
                        if parent_calldata.len() < 4 {
                            ::tracing::debug!(target: "brontes_classifier::discovery", ?deployed_address, ?factory, "invalid calldata length");
                            return Vec::new()
                        }

                        let mut key = [0u8; 24];
                        key[0..20].copy_from_slice(&**factory);
                        key[20..].copy_from_slice(&parent_calldata[0..4]);

                        #(
                            const #var_name: [u8; 24] = #fn_name();
                        )*

                        match key {
                            #(
                                #var_name => {
                                ::tracing::trace!(target: "brontes_classifier::discovery", ?deployed_address, ?factory, ?key, "match found");
                                return
                                    crate::FactoryDiscovery::decode_create_trace(
                                        &self.#i,
                                        tracer,
                                        deployed_address,
                                        trace_idx,
                                        parent_calldata,
                                    ).await
                                }
                            )*
                            _ => {
                                ::tracing::trace!(target: "brontes_classifier::discovery", ?deployed_address, ?factory, ?key, "no match found");
                                Vec::new()
                            }
                        }
                    }
                        })
                    .buffer_unordered(10)
                    .collect::<Vec<_>>()
                    .await
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>()
            }
        }
    ))
}

struct DiscoveryDispatch {
    // required for all
    struct_name: Ident,
    rest:        Vec<Ident>,
}
impl Parse for DiscoveryDispatch {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let struct_name: Ident = input.parse()?;
        let mut rest = Vec::new();
        while input.parse::<Token![,]>().is_ok() {
            rest.push(input.parse::<Ident>()?);
        }
        if !input.is_empty() {
            return Err(syn::Error::new(
                Span::call_site(),
                "no discovery implementations to dispatch to",
            ));
        }

        Ok(Self { rest, struct_name })
    }
}
