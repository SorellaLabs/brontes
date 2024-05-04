use quote::{quote, ToTokens};
use syn::ExprClosure;

pub struct ClosureDispatch {
    logs:        bool,
    call_data:   bool,
    return_data: bool,
    closure:     ExprClosure,
}

impl ClosureDispatch {
    pub fn new(logs: bool, call_data: bool, return_data: bool, closure: ExprClosure) -> Self {
        Self { closure, call_data, return_data, logs }
    }
}

impl ToTokens for ClosureDispatch {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let closure = &self.closure;

        let call_data = self
            .call_data
            .then_some(quote!(call_data,))
            .unwrap_or_default();

        let return_data = self
            .return_data
            .then_some(quote!(return_data,))
            .unwrap_or_default();

        let log_data = self.logs.then_some(quote!(log_data,)).unwrap_or_default();

        tokens.extend(quote!(
            let fixed_fields = call_info.get_fixed_fields();
            let result: ::eyre::Result<_> = (#closure)
            (
                fixed_fields,
                #call_data
                #return_data
                #log_data
                db_tx
            );

            // metrics
            if result.is_err() {
                let protocol= db_tx.get_protocol(call_info.target_address)?;
                crate::CLASSIFICATION_METRICS.get_or_init(|| brontes_metrics::classifier::ClassificationMetrics::default())
                    .bad_protocol_classification(protocol);
            }


            let result = result?;
        ))
    }
}
