use std::str::FromStr;

use itertools::Itertools;
use proc_macro2::{Literal, Span, TokenStream};
use quote::quote;
use syn::{braced, bracketed, parenthesized, parse::Parse, spanned::Spanned, token, ExprClosure, Ident, Index, LitByte, LitInt, Path, Token};

pub fn curve_discovery_impl(token_stream: TokenStream) -> syn::Result<TokenStream> {
    let parsed: CurveParse = syn::parse2(token_stream.into())?;


    let meta_pools = parsed.make_meta_pools();
    let plain_pools = parsed.make_plain_pools();

    let tokens = quote! {
        #meta_pools
        #plain_pools
    };

    Ok(tokens)
}

struct CurveParse {
    // required for all
    contract_protocol: Ident,
    abi_crate_path:    Path,
    meta_pool_impls:   u8,
    plain_pool_impls:  u8,
    factory_address:   Literal
}

impl CurveParse {


    fn make_meta_pools(&self) -> TokenStream {
        let full_contract_protocol = Ident::new(&format!("{}MetaPool", self.contract_protocol.to_string()), Span::call_site());
        let path = &self.abi_crate_path;
        let address = &self.factory_address;

        if self.meta_pool_impls == 1 {
            let decoder_name = Ident::new(&format!("{}MetaDiscovery", self.contract_protocol.to_string()), Span::call_site());
            let end_function_call_path = Ident::new(&format!("deploy_metapoolCall"), Span::call_site());
            let function_call_path = quote!(#path::#end_function_call_path);

            return quote! {
                discovery_impl!(
                    #decoder_name,
                    #function_call_path,
                    #address,
                    |deployed_address: Address, trace_index: u64, call: #end_function_call_path, tracer: Arc<T>| {
                        parse_meta_pool(Protocol::#full_contract_protocol, deployed_address, call._base_pool, call._coin, trace_index, tracer)
                    }
                );
            };
        } else {
            let impls = (0..self.meta_pool_impls)
                .into_iter()
                .map(|i| {
                    let decoder_name = Ident::new(&format!("{}MetaDiscovery{i}", self.contract_protocol.to_string()), Span::call_site());
                    let end_function_call_path = Ident::new(&format!("deploy_metapool_{i}Call"), Span::call_site());
                    let function_call_path = quote!(#path::#end_function_call_path);

                    quote! {
                        discovery_impl!(
                            #decoder_name,
                            #function_call_path,
                            #address,
                            |deployed_address: Address, trace_index: u64, call: #end_function_call_path, tracer: Arc<T>| {
                                parse_meta_pool(Protocol::#full_contract_protocol, deployed_address, call._base_pool, call._coin, trace_index, tracer)
                            }
                        );
                    }
                })
                .collect::<Vec<_>>();

            quote! {
                #(
                    #impls
                )*
            }
        }
    }

    fn make_plain_pools(&self) -> TokenStream {
        let full_contract_protocol = Ident::new(&format!("{}PlainPool", self.contract_protocol.to_string()), Span::call_site());
        let path = &self.abi_crate_path;
        let address = &self.factory_address;

        if self.plain_pool_impls == 1 {
            let decoder_name = Ident::new(&format!("{}PlainDiscovery", self.contract_protocol.to_string()), Span::call_site());
            let end_function_call_path = Ident::new(&format!("deploy_plain_poolCall"), Span::call_site());
            let function_call_path = quote!(#path::#end_function_call_path);

            return quote! {
                discovery_impl!(
                    #decoder_name,
                    #function_call_path,
                    #address,
                    |deployed_address: Address, trace_index: u64, call: #end_function_call_path, _| {
                        parse_plain_pool(Protocol::#full_contract_protocol, deployed_address, trace_index, call._coins)
                    }
                );
            };
        } else {
            let impls = (0..self.plain_pool_impls)
                .into_iter()
                .map(|i| {
                    let decoder_name = Ident::new(&format!("{}PlainDiscovery{i}", self.contract_protocol.to_string()), Span::call_site());
                    let end_function_call_path = Ident::new(&format!("deploy_plain_pool_{i}Call"), Span::call_site());
                    let function_call_path = quote!(#path::#end_function_call_path);

                    quote! {
                        discovery_impl!(
                            #decoder_name,
                            #function_call_path,
                            #address,
                            |deployed_address: Address, trace_index: u64, call: #end_function_call_path, _| {
                                parse_plain_pool(Protocol::#full_contract_protocol, deployed_address, trace_index, call._coins)
                            }
                        );
                    }
                })
                .collect::<Vec<_>>();

            quote! {
                #(
                    #impls
                )*
            }
        }
    }
}

impl Parse for CurveParse {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let contract_protocol: Ident = input
            .parse()
            .map_err(|e| syn::Error::new(e.span(), "Failed to parse decoder name"))?;
        input.parse::<Token![,]>()?;

        let abi_crate_path: Path = input
            .parse()
            .map_err(|e| syn::Error::new(e.span(), "Failed to parse path to function call"))?;
        input.parse::<Token![,]>()?;

        let factory_address = input.parse::<Literal>()?;
        input.parse::<Token![,]>()?;

        let content;
        parenthesized!(content in input);

        let meta_pool_impls: u8 = content.parse::<LitInt>()?.to_string().parse().unwrap();
        content.parse::<Token![,]>()?;

        let plain_pool_impls: u8 = content.parse::<LitInt>()?.to_string().parse().unwrap();

        if !input.is_empty() {
            return Err(syn::Error::new(input.span(), "There should be no values after the call function"))
        }

        Ok(Self { contract_protocol, abi_crate_path, meta_pool_impls, plain_pool_impls, factory_address })
    }
}

/*

    CurvecrvUSD,
    crate::raw::pools::impls::CurvecrvUSDFactory,
    addr
    (base, meta, plain)




*/
