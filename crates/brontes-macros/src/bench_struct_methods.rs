use proc_macro2::TokenStream;
use quote::quote;
use syn::{
    parse::{Parse, Parser},
    spanned::Spanned,
    Expr, ItemFn, MetaNameValue,
};
pub fn parse(item: ItemFn, attr: TokenStream) -> syn::Result<TokenStream> {
    // grab threads if specified
    let Some(field) = Parser::parse2(MetaNameValue::parse, attr.clone())
        .map(|name_val| {
            if name_val.path.segments.last()?.ident == "ptr" {
                let Expr::Field(ref a) = name_val.value else { return None };
                match &a.member {
                    syn::Member::Named(n) => Some(n.to_owned()),
                    _ => None,
                }
            } else {
                None
            }
        })
        .ok()
        .flatten()
    else {
        return Err(syn::Error::new(attr.span(), "invalid ptr to function call struct"));
    };

    let attrs = item.attrs;
    let vis = item.vis;
    let mut sig = item.sig;
    if sig.asyncness.is_some() {
        return Err(syn::Error::new(sig.asyncness.span(), "function must not be async"));
    }
    sig.asyncness = None;
    let block = item.block;

    let fn_name = sig.ident.to_string();

    Ok(quote!(
        #(#attrs)*
        #vis
        #sig
        {
            let start = ::std::time::Instant::now();
            let result = #block;
            let end = ::std::time::Instant::now();
            self.#field.add_bench(#fn_name.to_string(), end.duration_since(start));

            result
        }
    ))
}
