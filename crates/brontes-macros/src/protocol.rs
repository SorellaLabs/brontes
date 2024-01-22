use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{spanned::Spanned, DeriveInput, Fields};

pub fn to_const_byte(input: DeriveInput) -> syn::Result<TokenStream> {
    match input.data {
        syn::Data::Enum(ref e) => {
            for varient in &e.variants {
                if !matches!(varient.fields, Fields::Unit) {
                    return Err(syn::Error::new(
                        varient.fields.span(),
                        "Macro only works with all Unit fields",
                    ))
                }
            }
            if e.variants.len() > u8::MAX as usize {
                return Err(syn::Error::new(
                    Span::call_site(),
                    "macro only sports up to 255 elements",
                ))
            }

            let (const_value, name): (Vec<_>, Vec<_>) = e.variants.iter().enumerate().unzip();

            let enum_name = &input.ident;
            Ok(quote!(
                impl #enum_name {
                    const fn to_byte(self) -> u8 {
                        match self {
                            #(
                                Self::#name => #const_value as u8,
                            )*
                        }
                    }
                }

            ))
        }
        syn::Data::Struct(s) => {
            return Err(syn::Error::new(s.struct_token.span(), "only works with enums"))
        }
        syn::Data::Union(u) => {
            return Err(syn::Error::new(u.union_token.span(), "only works with enums"))
        }
    }
}
