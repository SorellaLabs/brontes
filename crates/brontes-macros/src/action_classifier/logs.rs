use itertools::{multizip, Itertools};
use proc_macro2::{Ident, Span, TokenStream};
use quote::{quote, ToTokens};
use syn::{Index, Path};

pub struct LogConfig {
    pub can_repeat:    bool,
    pub ignore_before: bool,
    pub log_ident:     Ident,
}

pub struct ParsedLogConfig {
    check_indexes:   Vec<Index>,
    is_repeatings:   Vec<bool>,
    ignore_befores:  Vec<bool>,
    log_field_names: Vec<Ident>,
    log_names:       Vec<Ident>,
}

pub struct LogData<'a> {
    exchange_name: &'a Ident,
    action_type:   &'a Ident,
    mod_path:      Path,
    log_config:    &'a [LogConfig],
}

impl<'a> LogData<'a> {
    pub fn new(
        exchange_name: &'a Ident,
        action_type: &'a Ident,
        fn_call_path: &'a Path,
        log_config: &'a [LogConfig],
    ) -> Self {
        let mut mod_path = fn_call_path.clone();
        mod_path.segments.pop().unwrap();
        mod_path.segments.pop_punct();

        Self { action_type, exchange_name, log_config, mod_path }
    }

    fn parse_log_config(&self) -> ParsedLogConfig {
        let (check_indexes, is_repeatings, ignore_befores, log_field_names, log_names): (
            Vec<_>,
            Vec<_>,
            Vec<_>,
            Vec<_>,
            Vec<_>,
        ) = self
            .log_config
            .iter()
            .enumerate()
            .map(|(i, LogConfig { can_repeat, log_ident, ignore_before })| {
                // is possible, need to increment count

                let idx = if *ignore_before { Index::from(0) } else { Index::from(i) };
                (
                    idx,
                    *can_repeat,
                    *ignore_before,
                    Ident::new(&(log_ident.to_string() + "_field"), Span::call_site()),
                    log_ident.clone(),
                )
            })
            .multiunzip();

        ParsedLogConfig { log_names, log_field_names, check_indexes, is_repeatings, ignore_befores }
    }

    fn generate_decoded_log_struct(
        &self,
        log_ident: &[Ident],
        log_field: &[Ident],
        log_repeating: &[bool],
    ) -> (TokenStream, Ident) {
        let mod_path = &self.mod_path;

        let log_return_struct_name =
            Ident::new(&(self.exchange_name.to_string() + "Logs"), Span::call_site());

        let log_return_builder_struct_name = Ident::new(
            &(self.exchange_name.to_string() + &self.action_type.to_string() + "Builder"),
            Span::call_site(),
        );

        let res_struct_fields = log_ident
            .iter()
            .zip(log_repeating.iter())
            .map(|(name, repeating)| {
                let field = Ident::new(&(name.to_string() + "_field"), Span::call_site());

                let data_type = if *repeating {
                    quote!(Vec<#mod_path::#name>)
                } else {
                    quote!(#mod_path::#name)
                };

                quote!(#field : #data_type)
            })
            .collect_vec();

        let return_struct_build_fields = log_ident
            .iter()
            .map(|name| {
                let field = Ident::new(&(name.to_string() + "_field"), Span::call_site());
                quote!(#field : self.#field.unwrap())
            })
            .collect_vec();

        let log_field_ty =
            log_repeating
                .iter()
                .zip(log_ident.iter())
                .map(|(repeating, name)| {
                    if *repeating {
                        quote!(Vec<#mod_path::#name>)
                    } else {
                        quote!(#mod_path::#name)
                    }
                })
                .collect_vec();

        (
            quote!(
                struct #log_return_builder_struct_name {
                    #(
                        #log_field: Option<#log_field_ty>
                    ),*
                }

                struct #log_return_struct_name {
                    #(#res_struct_fields),*
                }

                impl #log_return_builder_struct_name {
                    fn new() -> Self {
                        Self {
                            #(
                                #log_field: None
                            ),*
                        }
                    }

                    fn build(self) -> #log_return_struct_name {
                        #log_return_struct_name {
                            #(
                                #return_struct_build_fields
                            ),*
                        }
                    }
                }
            ),
            log_return_builder_struct_name,
        )
    }

    fn parse_ignore_before(
        &self,
        next_log: Option<&Ident>,
        log_name: &Ident,
        index: &Index,
        on_result: TokenStream,
    ) -> TokenStream {
        let mod_path = &self.mod_path;

        let has_next_log = if let Some(next_log) = next_log {
            quote!(
             if <#mod_path::#next_log
                as ::alloy_sol_types::SolEvent>
                    ::decode_log_data(&log.data, false).is_ok()
                    && started {
                    break
                }
            )
        } else {
            quote!()
        };

        quote!(
            let mut i = 0usize;
            let mut started = false;
            loop {
                if let Some(log) = &call_info.logs.get(#index + repeating_modifier + i) {
                    if let Ok(decoded_result) = <#mod_path::#log_name
                        as ::alloy_sol_types::SolEvent>
                            ::decode_log_data(&log.data, false) {
                            started = true;
                            #on_result
                    };

                    #has_next_log

                } else {
                    break
                }

                i += 1;
            }
            // move the index to where we finished
            repeating_modifier += i - 1;
        )
    }

    fn parse_different_paths(&self, config: &ParsedLogConfig) -> TokenStream {
        let ParsedLogConfig {
            check_indexes,
            log_field_names,
            log_names,
            is_repeatings,
            ignore_befores,
            ..
        } = config;

        let mod_path = &self.mod_path;
        let mut stream = TokenStream::new();

        for (enum_i, (indexes, log_field_name, log_name, repeating, ignore_before)) in
            multizip((check_indexes, log_field_names, log_names, is_repeatings, ignore_befores))
                .enumerate()
        {
            let res = if *repeating {
                if *ignore_before {
                    let next_log = log_names.get(enum_i + 1);

                    let parse = self.parse_ignore_before(
                        next_log,
                        log_name,
                        indexes,
                        quote!( log_result.push(decoded_result);),
                    );

                    quote!(
                        let mut log_result = Vec::new();
                        #parse
                        log_res.#log_field_name = Some(log_result);
                    )

                // repeating not ignore before
                } else {
                    quote!(
                        let mut repeating_results = Vec::new();
                        let mut i = 0usize;
                            let mut started = false;
                            loop {
                                if let Some(log) = &call_info.logs.get(
                                    #indexes + repeating_modifier + i) {
                                    if let Ok(decoded) =
                                        <#mod_path::#log_name as
                                        ::alloy_sol_types::SolEvent>
                                        ::decode_log_data(&log.data, false) {
                                            started = true;
                                            repeating_results.push(decoded);
                                    } else if started  {
                                        break
                                    }
                                } else {
                                    break
                                }

                                i += 1;
                            }

                            repeating_modifier += repeating_results.len();
                            log_res.#log_field_name = Some(repeating_results);
                    )
                }
            } else if *ignore_before {
                let next_log = log_names.get(enum_i + 1);
                self.parse_ignore_before(
                    next_log,
                    log_name,
                    indexes,
                    quote!(
                    log_res.#log_field_name = Some(decoded_result);
                    ),
                )
            } else {
                quote!(
                'possible: {
                        if let Some(log) = &call_info.logs.get(#indexes + repeating_modifier) {
                            if let Ok(decoded) = <#mod_path::#log_name
                                as ::alloy_sol_types::SolEvent>
                                ::decode_log_data(&log.data, false) {
                                    log_res.#log_field_name = Some(decoded);
                                    break 'possible
                            }
                            else {
                                ::tracing::error!(?call_info.from_address,
                                                  ?call_info.target_address,
                                                  ?self,
                                                  "decoding a default log failed, this should never occur,
                                                  please make a issue if you come across this"
                                );
                            }
                        }
                    }
                )
            };

            stream.extend(res);
        }

        stream
    }
}

impl ToTokens for LogData<'_> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let config = self.parse_log_config();
        let parsed_paths = self.parse_different_paths(&config);

        let ParsedLogConfig { log_field_names, log_names, is_repeatings, .. } = config;

        let (struct_parsing, log_builder_struct) =
            self.generate_decoded_log_struct(&log_names, &log_field_names, &is_repeatings);

        let log_result = quote!(
            #struct_parsing

            let mut log_res = #log_builder_struct::new();
            let mut repeating_modifier = 0usize;

            #parsed_paths

            let log_data = log_res.build();
        );

        tokens.extend(log_result)
    }
}
