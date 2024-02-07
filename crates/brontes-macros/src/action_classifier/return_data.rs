use quote::{quote, ToTokens};
use syn::Path;

pub struct ReturnData<'a> {
    path_to_call: &'a Path,
}

impl<'a> ReturnData<'a> {
    pub fn new(path_to_call: &'a Path) -> Self {
        Self { path_to_call }
    }
}

impl ToTokens for ReturnData<'_> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let path = &self.path_to_call;

        let call_tokens = quote!(
                let return_data = <#path
                    as alloy_sol_types::SolCall>
                ::abi_decode_returns(&call_info.return_data, false).map_err(|e| {
                    tracing::error!("return data failed to decode {:#?}", return_data);
                    e
                }).ok()?;
        );

        tokens.extend(call_tokens);
    }
}
