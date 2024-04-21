use proc_macro2::TokenStream;
use quote::quote;
use syn::{Ident, ItemStruct};
pub fn parse(item: ItemStruct) -> syn::Result<TokenStream> {
    let d_name = &item.ident;
    let name = Ident::new(&format!("{}Transposed", item.ident.to_string()), item.ident.span());

    let (f_name, f_type): (Vec<_>, Vec<_>) = item
        .fields
        .into_iter()
        .filter_map(|f| Some((f.ident?, f.ty)))
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
            #item

            impl From<Vec<#item>> for #transposed_struct {
                fn from(items: Vec<#item>) -> Self {
                    #(
                        let mut #f_name = Vec::new();
                    )*

                    for item in items {
                        #(
                            #f_name.push(item.#f_name);
                        )*
                    }

                    Self {
                        #f_name
                    }
                }

            }
    ))
}
