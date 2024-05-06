use proc_macro2::{Ident, TokenStream};
use quote::quote;
use syn::{parse::Parse, Expr, ItemFn, Token};

pub fn parse(item: ItemFn, attr: TokenStream) -> syn::Result<TokenStream> {
    // grab threads if specified
    let MetricList { ptr, fn_name, data } = syn::parse2(attr)?;

    let attrs = item.attrs;
    let vis = item.vis;
    let sig = item.sig;
    let block = item.block;

    Ok(quote!(
        #(#attrs)*
        #vis
        #sig
        {
            let result = #block;
            self.#ptr.#fn_name(#(#data),*);
            result
        }
    ))
}

pub struct MetricList {
    // ptr to metric in struct
    ptr:     Ident,
    // recorder name
    fn_name: Ident,
    data:    Vec<Expr>,
}

impl Parse for MetricList {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let ptr: Ident = input.parse()?;
        if ptr.to_string() != "ptr" {
            return Err(syn::Error::new(ptr.span(), "first field must be ptr=location"))
        }
        input.parse::<Token![=]>()?;
        let ptr_value: Ident = input.parse()?;

        input.parse::<Token![,]>()?;
        let fn_name: Ident = input.parse()?;

        let mut data = Vec::new();
        // take out all args
        while input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
            data.push(input.parse()?);
        }

        Ok(Self { ptr: ptr_value, fn_name, data })
    }
}
