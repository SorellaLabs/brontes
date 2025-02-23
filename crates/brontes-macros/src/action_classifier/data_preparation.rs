use proc_macro2::{Ident, TokenStream};
use quote::ToTokens;
use syn::{ExprClosure, Path};

use super::{
    call_data::CallData,
    closure_dispatch::ClosureDispatch,
    logs::{LogConfig, LogData},
    return_data::ReturnData,
};
pub struct CallDataParsing<'a> {
    call_data:   Option<CallData<'a>>,
    log_data:    Option<LogData<'a>>,
    return_data: Option<ReturnData<'a>>,
    closure:     ClosureDispatch,
}

impl<'a> CallDataParsing<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        logs: bool,
        call_data: bool,
        return_data: bool,
        include_delegated_logs: bool,
        exchange_name: &'a Ident,
        action_type: &'a Ident,
        fn_call_path: &'a Path,
        log_config: &'a [LogConfig],
        closure: ExprClosure,
    ) -> Self {
        let closure = ClosureDispatch::new(logs, call_data, return_data, closure);

        let log_data = if logs {
            Some(LogData::new(
                exchange_name,
                action_type,
                fn_call_path,
                log_config,
                include_delegated_logs,
            ))
        } else {
            None
        };
        let call_data = if call_data { Some(CallData::new(fn_call_path)) } else { None };
        let return_data = if return_data { Some(ReturnData::new(fn_call_path)) } else { None };

        Self { call_data, return_data, log_data, closure }
    }
}

impl ToTokens for CallDataParsing<'_> {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        if let Some(call_data) = &self.call_data {
            call_data.to_tokens(tokens)
        }
        if let Some(log_data) = &self.log_data {
            log_data.to_tokens(tokens)
        }
        if let Some(return_data) = &self.return_data {
            return_data.to_tokens(tokens);
        }
        self.closure.to_tokens(tokens);
    }
}
