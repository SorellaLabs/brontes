use quote::{quote, ToTokens};
use syn::Path;

pub struct CallData<'a> {
    path_to_call: &'a Path,
}

impl<'a> CallData<'a> {
    pub fn new(path_to_call: &'a Path) -> Self {
        Self { path_to_call }
    }
}

impl ToTokens for CallData<'_> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let path = &self.path_to_call;

        let call_tokens = quote!(
            let call_data = <#path
                as ::alloy_sol_types::SolCall>::abi_decode(&call_info.call_data[..], false)?;
        );

        tokens.extend(call_tokens);
    }
}
