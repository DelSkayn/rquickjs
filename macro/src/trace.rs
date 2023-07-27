use proc_macro2::TokenStream;
use proc_macro_error::abort;
use quote::{format_ident, quote};
use syn::{Data, DataEnum, DataStruct, DeriveInput};

use crate::{
    common::{add_js_lifetime, crate_ident},
    fields::Field,
};

pub(crate) fn expand(input: DeriveInput) -> TokenStream {
    let DeriveInput {
        ident,
        generics,
        data,
        ..
    } = input;

    let lifetime_generics = add_js_lifetime(&generics);
    let lib_crate = crate_ident();

    match data {
        Data::Struct(struct_) => {
            let DataStruct { fields, .. } = struct_;

            let parsed_fields = Field::parse_fields(&fields);
            let trace_impls = parsed_fields
                .iter()
                .enumerate()
                .map(|(idx, f)| f.expand_trace_body(&lib_crate, idx));

            quote! {
                impl #lifetime_generics #lib_crate::class::Trace<'js> for #ident #generics{
                    fn trace<'a>(&self, _tracer: #lib_crate::class::Tracer<'a,'js>){
                        #(#trace_impls)*
                    }
                }
            }
        }
        Data::Enum(DataEnum { variants, .. }) => {
            let body = variants.iter().map(|x| {
                let ident = &x.ident;
                let fields = Field::parse_fields(&x.fields);
                match x.fields {
                    syn::Fields::Named(_) => {
                        let mut has_skip = false;
                        let field_names = fields
                            .iter()
                            .filter_map(|f| {
                                if f.skip_trace {
                                    has_skip = true;
                                    None
                                } else {
                                    Some(&f.ident)
                                }
                            })
                            .collect::<Vec<_>>();

                        let pattern = if has_skip {
                            quote! {
                                Self::#ident{ #(ref #field_names,)* .. }
                            }
                        } else {
                            quote! {
                                Self::#ident{ #(ref #field_names),* }
                            }
                        };

                        let body = quote! {
                            {
                                #(#lib_crate::class::Trace::trace(#field_names, _tracer);)*
                            }
                        };

                        quote! {
                            #pattern => #body
                        }
                    }
                    syn::Fields::Unnamed(_) => {
                        let patterns = fields.iter().enumerate().map(|(idx, f)| {
                            if f.skip_trace {
                                quote!(_)
                            } else {
                                let ident = format_ident!("tmp_{idx}");
                                quote!(ref #ident)
                            }
                        });
                        let pattern = quote!(Self::#ident(#(#patterns),*));

                        let names = fields.iter().enumerate().filter_map(|(idx, f)| {
                            if f.skip_trace {
                                None
                            } else {
                                let ident = format_ident!("tmp_{idx}");
                                Some(ident)
                            }
                        });
                        let body = quote! {
                            {
                                #(#lib_crate::class::Trace::trace(#names,_tracer);)*
                            }
                        };

                        quote! {
                            #pattern => #body
                        }
                    }
                    syn::Fields::Unit => {
                        quote! {
                            Self::#ident => {}
                        }
                    }
                }
            });

            quote! {
                impl #lifetime_generics #lib_crate::class::Trace<'js> for #ident #generics {
                    fn trace<'a>(&self, _tracer: #lib_crate::class::Tracer<'a,'js>){
                        match *self{
                            #(#body,)*
                        }
                    }
                }
            }
        }
        Data::Union(u) => {
            abort!(u.union_token, "deriving trace for unions is not supported");
        }
    }
}
