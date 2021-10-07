use super::{DataField, DataType};
use crate::{Config, Ident, TokenStream};
use darling::ast::{Data, Fields, Style};
use quote::{format_ident, quote};
use syn::{parse_quote, Index};

pub struct HasRefs {
    config: Config,
}

impl HasRefs {
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    pub fn expand(&self, input: &DataType) -> TokenStream {
        let lib_crate = &self.config.lib_crate;
        let ident = &input.ident;
        let impl_params = input.impl_params(false);
        let type_name = input.type_name();
        let where_clause = input.where_clause(Some(parse_quote!(T: #lib_crate::HasRefs)), None);

        use Data::*;
        let body = match &input.data {
            Struct(fields) => self.expand_fields(ident, None, fields),
            Enum(variants) => {
                let bodies = variants.iter().map(|variant| {
                    self.expand_fields(ident, Some(&variant.ident), &variant.fields)
                });
                quote! {
                    match self {
                        #(#bodies)*
                    }
                }
            }
        };

        let impl_params = if impl_params.is_empty() {
            quote!()
        } else {
            quote!(<#impl_params>)
        };

        quote! {
            impl #impl_params #lib_crate::HasRefs for #type_name #where_clause {
                fn mark_refs(&self, _marker: &#lib_crate::RefsMarker) {
                    #body
                }
            }
        }
    }

    fn expand_fields(
        &self,
        ident: &Ident,
        variant: Option<&Ident>,
        fields: &Fields<DataField>,
    ) -> TokenStream {
        let lib_crate = &self.config.lib_crate;

        use Style::*;
        match fields.style {
            Unit => {
                if let Some(variant) = variant {
                    // variant
                    quote! {
                        #ident::#variant => {}
                    }
                } else {
                    // struct
                    quote! {}
                }
            }
            Struct => {
                let field_idents = fields
                    .fields
                    .iter()
                    .filter(|field| field.has_refs)
                    .map(|field| field.ident.as_ref().unwrap())
                    .collect::<Vec<_>>();

                if let Some(variant) = variant {
                    // variant
                    let rest = if field_idents.len() < fields.fields.len() {
                        quote! { .. }
                    } else {
                        quote! {}
                    };
                    quote! {
                        #ident::#variant { #(#field_idents,)* #rest } => {
                            #(#lib_crate::HasRefs::mark_refs(#field_idents, _marker);)*
                        }
                    }
                } else {
                    // struct
                    quote! {
                        #(#lib_crate::HasRefs::mark_refs(&self.#field_idents, _marker);)*
                    }
                }
            }
            Tuple => {
                if let Some(variant) = variant {
                    // variant
                    let field_patterns = fields.fields.iter().enumerate().map(|(index, field)| {
                        if field.has_refs {
                            format_ident!("_{}", index)
                        } else {
                            format_ident!("_")
                        }
                    });

                    let field_aliases = fields
                        .fields
                        .iter()
                        .enumerate()
                        .filter(|(_, field)| field.has_refs)
                        .map(|(index, _)| format_ident!("_{}", index));

                    quote! {
                        #ident::#variant(#(#field_patterns),*) => {
                            #(#lib_crate::HasRefs::mark_refs(#field_aliases, _marker);)*
                        }
                    }
                } else {
                    // struct
                    let field_indexes = fields
                        .fields
                        .iter()
                        .enumerate()
                        .filter(|(_, field)| field.has_refs)
                        .map(|(index, _)| Index::from(index));

                    quote! {
                        #(#lib_crate::HasRefs::mark_refs(&self.#field_indexes, _marker);)*
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod test {
    test_cases! {
        struct_with_refs HasRefs {
            struct Data {
                #[quickjs(has_refs)]
                lists: HashMap<String, Persistent<Array<'static>>>,
                #[quickjs(has_refs)]
                func: Persistent<Function<'static>>,
                flag: bool,
                text: String,
            }
        } {
            impl rquickjs::HasRefs for Data {
                fn mark_refs(&self, _marker: &rquickjs::RefsMarker) {
                    rquickjs::HasRefs::mark_refs(&self.lists, _marker);
                    rquickjs::HasRefs::mark_refs(&self.func, _marker);
                }
            }
        };

        enum_with_refs HasRefs {
            enum Data {
                Lists(
                    #[quickjs(has_refs)]
                    HashMap<String, Persistent<Array<'static>>>
                ),
                Func {
                    name: String,
                    #[quickjs(has_refs)]
                    func: Persistent<Function<'static>>,
                },
                Flag(bool),
                Text(String),
            }
        } {
            impl rquickjs::HasRefs for Data {
                fn mark_refs(&self, _marker: &rquickjs::RefsMarker) {
                    match self {
                        Data::Lists(_0) => {
                            rquickjs::HasRefs::mark_refs(_0, _marker);
                        }
                        Data::Func { func, .. } => {
                            rquickjs::HasRefs::mark_refs(func, _marker);
                        }
                        Data::Flag(_) => { }
                        Data::Text(_) => { }
                    }
                }
            }
        };
    }
}
