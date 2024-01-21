use core::panic;

use proc_macro::TokenStream;
use proc_macro2::Literal;
use quote::quote;
use syn::{parse::Parse, ExprClosure, Ident, Index, Path, Token};

pub fn discovery_impl(token_stream: TokenStream) -> TokenStream {
    let MacroParse { decoder_name, function_call_path, factory_address, address_call_function } =
        syn::parse2(token_stream.into()).unwrap();

    assert_address(&factory_address);
    let stripped_address = &factory_address.to_string()[2..];
    let decoder_name_str = decoder_name.to_string();

    quote! (
        use #function_call_path;

        #[derive(Debug, Default)]
        pub struct #decoder_name;

        impl crate::FactoryDecoder for #decoder_name {
            fn address_and_function_selector(&self) -> [u8; 24] {
                let mut result = [0u8; 24];
                result[0..20].copy_from_slice(&::alloy_primitives::hex!(#stripped_address));
                result[20..].copy_from_slice(&<#function_call_path
                                             as ::alloy_sol_types::SolCall>::SELECTOR);

                result
            }

            fn decode_new_pool (
                &self,
                deployed_address: ::alloy_primitives::Address,
                parent_calldata: ::alloy_primitives::Bytes,
            ) -> Vec<::brontes_pricing::types::DiscoveredPool> {
                let Ok(decoded_data) = <#function_call_path
                    as ::alloy_sol_types::SolCall>::abi_decode(&parent_calldata[..], false)
                    else {
                        ::tracing::error!("{} failed to decode calldata", #decoder_name_str);
                        return Vec::new();
                };
                (#address_call_function)(deployed_address, decoded_data)
            }
        }
    )
    .into()
}

fn assert_address(possible_address: &Literal) -> bool {
    let stred = possible_address.to_string();
    if !stred.starts_with("0x") {
        panic!("given factory address is invalid. Needs to start with 0x");
    }
    if stred.len() != 42 {
        panic!("given factory address length is incorrect");
    }

    true
}

struct MacroParse {
    // required for all
    decoder_name:          Ident,
    function_call_path:    Path,
    factory_address:       Literal,
    /// The closure that we use to get the address of the pool
    address_call_function: ExprClosure,
}

impl Parse for MacroParse {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let decoder_name: Ident = input.parse()?;
        input.parse::<Token![,]>()?;
        let function_call_path: Path = input.parse()?;
        input.parse::<Token![,]>()?;
        let factory_address: Literal = input.parse()?;
        input.parse::<Token![,]>()?;
        let address_call_function: ExprClosure = input.parse()?;

        if !input.is_empty() {
            return Err(syn::Error::new(
                input.span(),
                "There should be no values after the call function",
            ))
        }

        Ok(Self { decoder_name, factory_address, function_call_path, address_call_function })
    }
}

pub fn discovery_dispatch(input: TokenStream) -> TokenStream {
    let DiscoveryDispatch { struct_name, rest } = syn::parse2(input.into()).unwrap();

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
            fn dispatch(
                    factory: ::alloy_primitives::Address,
                    deployed_address: ::alloy_primitives::Address,
                    parent_calldata: ::alloy_primitives::Bytes,
                ) -> Vec<::brontes_pricing::types::DiscoveredPool> {
                if parent_calldata.len() < 4 {
                    ::tracing::warn!(?deployed_address, ?factory, "invalid calldata length");
                    return Vec::new()
                }

                let mut key = [0u8; 24];
                key[0..20].copy_from_slice(&**factory);
                key[0..4].copy_from_slice(&parent_calldata[0..4]);


                let this = Self::default();

                if key == crate::FactoryDecoder::address_and_function_selector(&this.0) {
                    return
                        crate::FactoryDecoder::decode_new_pool(
                            &this.0,
                            deployed_address,
                            parent_calldata,
                        )
                }

                #( else if key == crate::FactoryDecoder::address_and_function_selector(&this.#i) {
                        return crate::FactoryDecoder::decode_new_pool(
                            &this.#i,
                            deployed_address,
                            parent_calldata,
                        )
                    }
                )*

                Vec::new()
            }
        }
    )
    .into()
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
            panic!("unkown characters")
        }

        Ok(Self { rest, struct_name })
    }
}
