use proc_macro2::TokenStream;
use quote::quote;
use syn::{Ident, ItemStruct};

pub fn parse(item: ItemStruct) -> syn::Result<TokenStream> {
    let d_name = &item.ident;
    let name = Ident::new(&format!("{}Transposed", item.ident.to_string()), item.ident.span());

    let (f_name, f_type): (Vec<_>, Vec<_>) = item
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
            #item

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
