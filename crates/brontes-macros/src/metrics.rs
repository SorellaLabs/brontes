use proc_macro2::TokenStream;
use quote::quote;
use syn::{
    punctuated::Punctuated, spanned::Spanned, token::Comma, Data, DeriveInput, Ident, Variant,Fields
};

pub fn parse(item: DeriveInput) -> syn::Result<TokenStream> {
    let data = if let Data::Enum(ref i) = item.data {
        i
    } else {
        return Err(syn::Error::new(item.span(), "only supports enum"))
    };

    let from_impls_for_structs = generate_from_impls(&item.ident, &data.variants)?;

    Ok(quote!())
}

pub fn generate_from_impls(enum_name: &Ident, varients: &Punctuated<Variant, Comma>) -> syn::Result<TokenStream> {
    let mut idents = vec![];
    let mut values = vec![];

    for var in varients {
        let Fields::Unnamed(name) = &var.fields else { 
            return Err(syn::Error::new(var.fields.span(), "only supports unnamed field"));
        };
        idents.push(&var.ident);
        values.push(name);
    }

    Ok(quote!(
        #(
            impl From<#values> for #enum_name {
                fn from(v: #values) -> #enum_name {
                    #enum_name::#idents(v)
                }
            }
        )*

    ))
}
