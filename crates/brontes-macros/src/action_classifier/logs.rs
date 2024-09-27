use itertools::{multizip, Itertools};
use proc_macro2::{Ident, Span, TokenStream};
use quote::{quote, ToTokens};
use syn::{Index, Path};

#[derive(Debug)]
pub struct LogConfig {
    pub can_repeat:    bool,
    pub ignore_before: bool,
    pub log_ident:     Ident,
    // might as well make n amount if we already need 1 fallback
    pub log_fallbacks: Vec<Ident>,
}

pub struct ParsedLogConfig {
    check_indexes:   Vec<Index>,
    is_repeatings:   Vec<bool>,
    ignore_befores:  Vec<bool>,
    log_field_names: Vec<Vec<Ident>>,
    log_names:       Vec<Vec<Ident>>,
}

pub struct LogData<'a> {
    exchange_name:          &'a Ident,
    action_type:            &'a Ident,
    mod_path:               Path,
    log_config:             &'a [LogConfig],
    include_delegated_logs: bool,
}

impl<'a> LogData<'a> {
    pub fn new(
        exchange_name: &'a Ident,
        action_type: &'a Ident,
        fn_call_path: &'a Path,
        log_config: &'a [LogConfig],
        include_delegated_logs: bool,
    ) -> Self {
        let mut mod_path = fn_call_path.clone();
        mod_path.segments.pop().unwrap();
        mod_path.segments.pop_punct();

        Self { action_type, exchange_name, log_config, mod_path, include_delegated_logs }
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
            .map(|(i, LogConfig { can_repeat, log_ident, ignore_before, log_fallbacks })| {
                // is possible, need to increment count

                let idx = if *ignore_before { Index::from(0) } else { Index::from(i) };
                (
                    idx,
                    *can_repeat,
                    *ignore_before,
                    vec![log_ident]
                        .into_iter()
                        .chain(log_fallbacks)
                        .map(|log_ident| {
                            Ident::new(&(log_ident.to_string() + "_field"), Span::call_site())
                        })
                        .collect::<Vec<_>>(),
                    vec![log_ident.clone()]
                        .into_iter()
                        .chain(log_fallbacks.clone())
                        .collect::<Vec<_>>(),
                )
            })
            .multiunzip();

        ParsedLogConfig { log_names, log_field_names, check_indexes, is_repeatings, ignore_befores }
    }

    fn generate_decoded_log_struct(
        &self,
        log_ident: &[Vec<Ident>],
        log_field: &[Vec<Ident>],
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
            .flat_map(|(names, repeating)| {
                names
                    .iter()
                    .map(|name| {
                        let field = Ident::new(&(name.to_string() + "_field"), Span::call_site());

                        let data_type = if *repeating {
                            quote!(Vec<#mod_path::#name>)
                        } else {
                            quote!(#mod_path::#name)
                        };

                        quote!([<#field:snake>]: ::eyre::Result<#data_type>)
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();

        let return_struct_build_fields = log_ident
            .iter()
            .flat_map(|names| {
                names
                    .iter()
                    .map(|name| {
                        let field = Ident::new(&(name.to_string() + "_field"), Span::call_site());
                        let message = format!(
                            "logs are not setup properly for this macro as the requested log {} \
                             was not found",
                            name
                        );

                        quote!([<#field:snake>]: self.[<#field:snake>].ok_or_else(|| {
                                ::tracing::warn!(?call_info, "{}", #message);
                                ::eyre::eyre!("call_info: {:?}, {}",call_info, #message)
                        }))
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();

        let log_field_ty = log_repeating
            .iter()
            .zip(log_ident.iter())
            .flat_map(|(repeating, names)| {
                names
                    .iter()
                    .map(|name| {
                        if *repeating {
                            quote!(Vec<#mod_path::#name>)
                        } else {
                            quote!(#mod_path::#name)
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();

        let log_field = log_field.iter().flatten().collect::<Vec<_>>();

        (
            quote!(
                ::paste::paste!(
                    #[allow(non_camel_case_types)]
                    struct [<#log_return_builder_struct_name:camel>] {
                        #(
                            [<#log_field:snake>]: Option<#log_field_ty>
                        ),*
                    }

                    #[allow(non_camel_case_types)]
                    struct #log_return_struct_name {
                        #(#res_struct_fields),*
                    }

                    impl [<#log_return_builder_struct_name:camel>] {
                        fn new() -> Self {
                            Self {
                                #(
                                    [<#log_field:snake>]: None
                                ),*
                            }
                        }

                        fn build(
                            self,
                            call_info: &::brontes_types::structured_trace::CallFrameInfo<'_>
                            )
                            -> #log_return_struct_name {
                                #log_return_struct_name {
                                #(
                                    #return_struct_build_fields
                                ),*
                            }
                        }
                    }
                );
            ),
            log_return_builder_struct_name,
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

        let mut stream = TokenStream::new();

        if self.include_delegated_logs {
            stream.extend(quote!(
                 let mut merged_logs: Vec<&alloy_primitives::Log> =
                     Vec::with_capacity(call_info.logs.len() + call_info.delegate_logs.len());
                 merged_logs.extend(call_info.logs.iter());
                 merged_logs.extend(call_info.delegate_logs.iter());

                 let logs = merged_logs.as_slice();
            ));
        } else {
            stream.extend(quote!(
                let logs = call_info.logs;
            ));
        }

        for (enum_i, (indexes, log_field_name, log_name, repeating, ignore_before)) in
            multizip((check_indexes, log_field_names, log_names, is_repeatings, ignore_befores))
                .enumerate()
        {
            let res = match (*repeating, *ignore_before) {
                (true, true) => self.parse_repeating_and_ignore_before(
                    enum_i,
                    log_names,
                    log_name,
                    log_field_name,
                    indexes,
                ),
                (true, false) => self.parse_repeating(log_name, log_field_name, indexes),
                (false, true) => {
                    let next_log = log_names.get(enum_i + 1);
                    self.parse_ignore_before(
                        next_log,
                        log_name,
                        indexes,
                        log_field_name
                            .iter()
                            .map(|field| {
                                quote!(
                                    ::paste::paste!(
                                        log_res.[<#field:snake>] = Some(decoded_result);
                                    );
                                )
                            })
                            .collect::<Vec<_>>(),
                    )
                }
                (false, false) => self.parse_default(log_name, log_field_name, indexes),
            };

            stream.extend(res);
        }

        stream
    }

    fn parse_ignore_before(
        &self,
        next_log: Option<&Vec<Ident>>,
        log_name: &[Ident],
        index: &Index,
        on_result: Vec<TokenStream>,
    ) -> TokenStream {
        let mod_path = &self.mod_path;

        let has_next_log = if let Some(next_log) = next_log {
            quote!(
                #(
                 if <#mod_path::#next_log
                    as ::alloy_sol_types::SolEvent>
                        ::decode_log_data(&log.data, false).is_ok()
                        && started {
                        break
                    }
                )*
            )
        } else {
            quote!()
        };

        quote!(
            let mut i = 0usize;
            let mut started = false;
            loop {
                if let Some(log) = &logs.get(#index + repeating_modifier + i) {
                    #(
                        if let Ok(decoded_result) = <#mod_path::#log_name
                            as ::alloy_sol_types::SolEvent>
                                ::decode_log_data(&log.data, false) {
                                started = true;
                                #on_result
                        };
                    )*

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

    fn parse_repeating_and_ignore_before(
        &self,
        enum_i: usize,
        log_names: &[Vec<Ident>],
        log_name: &[Ident],
        log_field_name: &[Ident],
        indexes: &Index,
    ) -> TokenStream {
        let next_log = log_names.get(enum_i + 1);
        let parse = self.parse_ignore_before(
            next_log,
            log_name,
            indexes,
            log_field_name
                .iter()
                .map(|field| {
                    quote!(
                        ::paste::paste!(
                            [<#field:snake _res>].push(decoded_result);
                        );
                    )
                })
                .collect::<Vec<_>>(),
        );
        quote!(
            #(
                ::paste::paste!(
                    let mut [<#log_field_name:snake _res>] = Vec::new();
                );
            )*

            #parse
            #(
                ::paste::paste!(
                    log_res.[<#log_field_name:snake>] = Some([<#log_field_name:snake _res>]);
                );
            )*
        )
    }

    fn parse_repeating(
        &self,
        log_name: &[Ident],
        log_field_name: &[Ident],
        indexes: &Index,
    ) -> TokenStream {
        let mod_path = &self.mod_path;
        quote!(
            #(
                ::paste::paste!(
                    let mut [<#log_field_name:snake _res>] = Vec::new();
                );
            )*

            let mut i = 0usize;
                let mut started = false;
                loop {
                    if let Some(log) = &logs.get(
                        #indexes + repeating_modifier + i) {

                        let mut any_parsed = false;
                        #(
                            if let Ok(decoded) =
                                <#mod_path::#log_name as
                                ::alloy_sol_types::SolEvent>
                                ::decode_log_data(&log.data, false) {
                                    started = true;
                                    any_parsed = true;
                                    ::paste::paste!(
                                        [<#log_field_name:snake _res>].push(decoded);
                                    );
                            }
                        )*

                        if started  && !any_parsed {
                            break
                        }

                    } else {
                        // no log found
                        break
                    }

                    i += 1;
                }

                #(
                    ::paste::paste!(
                        repeating_modifier + = [<#log_field_name:snake _res>].len();
                        log_res.[<#log_field_name:snake>] = Some([<#log_field_name:snake _res>]);
                    );
                )*
        )
    }

    fn parse_default(
        &self,
        log_name: &[Ident],
        log_field_name: &[Ident],
        indexes: &Index,
    ) -> TokenStream {
        let mod_path = &self.mod_path;
        quote!(
        'possible: {
                if let Some(log) = &logs.get(#indexes + repeating_modifier) {
                    ::paste::paste!(
                    #(
                        if let Ok(decoded) = <#mod_path::#log_name
                            as ::alloy_sol_types::SolEvent>
                            ::decode_log_data(&log.data, false) {
                                log_res.[<#log_field_name:snake>] = Some(decoded);
                                break 'possible
                        }
                    )*
                    );

                    ::tracing::warn!(?call_info,
                                      ?self,
                                      "decoding a default log failed, this should never occur,
                                      please make a issue if you come across this"
                    );
                }
            }
        )
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
            if call_info.logs.is_empty() && call_info.delegate_logs.is_empty() {
                ::tracing::error!(?call_info, "tried to decode using logs when no logs where found \
                                  for call");
            }
            #struct_parsing

            paste::paste!(
                let mut log_res = [<#log_builder_struct:camel>]::new();
            );
            let mut repeating_modifier = 0usize;

            #parsed_paths

            let log_data = log_res.build(&call_info);
        );

        tokens.extend(log_result)
    }
}
