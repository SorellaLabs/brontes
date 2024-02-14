use proc_macro2::TokenStream;
use quote::quote;
use syn::{spanned::Spanned, ItemFn};

pub fn parse(item: ItemFn) -> syn::Result<TokenStream> {
    let attrs = item.attrs;
    let vis = item.vis;
    let mut sig = item.sig;
    if sig.asyncness.is_none() {
        return Err(syn::Error::new(
            sig.asyncness.span(),
            "function must be async",
        ));
    }
    sig.asyncness = None;
    let block = item.block;

    Ok(quote!(
        #[test]
        #(#attrs)*
        #vis
        #sig
        {
            std::thread::spawn(move || {
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .worker_threads(2)
                .build()
                .unwrap()
                .block_on(async move #block)

            }).join().unwrap();
        }
    ))
}
