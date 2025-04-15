use std::str::FromStr;

use proc_macro2::TokenStream;
use quote::quote;
use syn::{
    parse::{Parse, Parser},
    spanned::Spanned,
    Expr, ItemFn, MetaNameValue,
};

pub fn parse(item: ItemFn, attr: TokenStream) -> syn::Result<TokenStream> {
    // grab threads if specified
    let threads = Parser::parse2(MetaNameValue::parse, attr)
        .map(|name_val| {
            if name_val.path.segments.last()?.ident == "threads" {
                let Expr::Lit(ref a) = name_val.value else { return None };
                match &a.lit {
                    syn::Lit::Int(i) => Some(usize::from_str(i.base10_digits()).unwrap()),
                    _ => None,
                }
            } else {
                None
            }
        })
        .ok()
        .flatten()
        .unwrap_or(3);

    let attrs = item.attrs;
    let vis = item.vis;
    let mut sig = item.sig;
    if sig.asyncness.is_none() {
        return Err(syn::Error::new(sig.asyncness.span(), "function must be async"))
    }
    sig.asyncness = None;
    let block = item.block;

    Ok(quote!(
        #[test]
        #(#attrs)*
        #vis
        #sig
        {
            dotenv::dotenv().expect("failed to load env");
            ::brontes_core::test_utils::init_tracing();
            ::brontes_types::wait_for_tests(#threads, || {
                std::thread::spawn(move || {
                tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .worker_threads(#threads)
                    .build()
                    .unwrap()
                    .block_on(async move #block)

                }).join().unwrap();
            });
        }
    ))
}
