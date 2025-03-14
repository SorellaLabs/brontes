use proc_macro2::TokenStream;
use quote::quote;
use syn::{spanned::Spanned, Data, DeriveInput, Ident};

pub fn parse(item: DeriveInput) -> syn::Result<TokenStream> {
    let data = if let Data::Struct(ref i) = item.data {
        i
    } else {
        return Err(syn::Error::new(item.span(), "only supports structs"));
    };

    let d_name = &item.ident;
    let name = Ident::new(&format!("{}Transposed", item.ident), item.ident.span());

    let (f_name, f_type): (Vec<_>, Vec<_>) = data
        .fields
        .iter()
        .filter_map(|f| Some((f.ident.as_ref()?, f.ty.clone())))
        .unzip();

    let transposed_struct = quote!(
        pub struct #name {
            #(
                pub #f_name: Vec<#f_type>,
            )*
        }
    );

    Ok(quote!(
            #transposed_struct

            impl From<Vec<#d_name>> for #name {
                fn from(_i: Vec<#d_name>) -> Self {
                    #(
                        let mut #f_name = Vec::new();
                    )*

                    for _items in _i {
                        #(
                            #f_name.push(_items.#f_name);
                        )*
                    }

                    Self {
                        #(
                            #f_name,
                        )*
                    }
                }

            }
    ))
}
