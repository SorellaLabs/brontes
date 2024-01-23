use itertools::Itertools;
use proc_macro2::{Ident, Span, TokenStream};
use quote::{quote, ToTokens};
use syn::{Index, LitBool, Path};

pub struct LogData<'a> {
    exchange_name: &'a Ident,
    action_type:   &'a Ident,
    mod_path:      Path,
    log_config:    &'a [(bool, bool, Ident)],
}

impl<'a> LogData<'a> {
    pub fn new(
        exchange_name: &'a Ident,
        action_type: &'a Ident,
        fn_call_path: &'a Path,
        log_config: &'a [(bool, bool, Ident)],
    ) -> Self {
        let mut mod_path = fn_call_path.clone();
        mod_path.segments.pop().unwrap();
        mod_path.segments.pop_punct();

        Self { action_type, exchange_name, log_config, mod_path }
    }

    fn parse_log_config(&self) -> (Vec<Vec<Index>>, Vec<LitBool>, Vec<Ident>, Vec<Ident>) {
        let mut is_possible_count = 0usize;
        self.log_config
            .into_iter()
            .enumerate()
            .collect_vec()
            .into_iter()
            .filter_map(|(i, n)| {
                // is possible, need to increment count
                if n.0 {
                    is_possible_count += 1;
                }
                if n.1 {
                    return None
                }

                Some((
                    (0..=is_possible_count)
                        .into_iter()
                        .filter_map(|shift| {
                            if i < shift {
                                return None
                            }
                            Some(Index::from(i - shift))
                        })
                        .collect_vec(),
                    LitBool::new(n.0, Span::call_site()),
                    Ident::new(&(n.2.to_string() + "_field"), Span::call_site()),
                    n.2.clone(),
                ))
            })
            .multiunzip()
    }

    fn generate_decoded_log_struct(
        &self,
        log_ident: &[Ident],
        log_field: &[Ident],
        log_optional: &[LitBool],
    ) -> (TokenStream, Ident) {
        let mod_path = &self.mod_path;

        let log_return_struct_name = Ident::new(
            &(self.exchange_name.to_string() + &self.action_type.to_string()),
            Span::call_site(),
        );

        let log_return_builder_struct_name = Ident::new(
            &(self.exchange_name.to_string() + &self.action_type.to_string() + "Builder"),
            Span::call_site(),
        );

        let res_struct_fields = log_optional
            .iter()
            .zip(log_ident.iter())
            .filter_map(|(optional, res)| {
                let field = Ident::new(&(res.to_string() + "_field"), Span::call_site());

                Some(if optional.value {
                    quote!(#field : Option<#mod_path::#res>)
                } else {
                    quote!(#field : #mod_path::#res)
                })
            })
            .collect_vec();

        let return_struct_build_fields = log_optional
            .iter()
            .zip(log_ident.iter())
            .filter_map(|(optional, res)| {
                let field = Ident::new(&(res.to_string() + "_field"), Span::call_site());

                Some(if optional.value {
                    // don't unwrap optional
                    quote!(#field : self.#field)
                } else {
                    quote!(#field : self.#field.unwrap())
                })
            })
            .collect_vec();

        (
            quote!(
                struct #log_return_builder_struct_name {
                    #(
                        #log_field: Option<#mod_path::#log_ident>
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
}

impl ToTokens for LogData<'_> {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        let (log_idx, log_optional, log_field, log_ident) = self.parse_log_config();
        let (struct_parsing, log_builder_struct) =
            self.generate_decoded_log_struct(&log_ident, &log_field, &log_optional);

        let mod_path = &self.mod_path;
        let log_result = quote!(
            #struct_parsing

            let mut log_res = #log_builder_struct::new();
            #(
                'possible: {
                #(
                    if let Some(log) = &logs.get(#log_idx) {
                        if let Some(decoded)= <#mod_path::#log_ident
                            as ::alloy_sol_types::SolEvent>
                            ::decode_log_data(&log.data, false).ok() {
                                log_res.#log_field = Some(decoded);
                                break 'possible
                            }
                    }
                )*
                }
            )*
            let log_data = log_res.build();
        );

        tokens.extend(log_result)
    }
}
