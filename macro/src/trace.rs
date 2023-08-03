use proc_macro2::TokenStream;
use proc_macro_error::abort;
use quote::{format_ident, quote};
use syn::{Data, DataEnum, DataStruct, DeriveInput};

use crate::{
    common::{add_js_lifetime, crate_ident},
    fields::Fields,
};

pub(crate) fn expand(input: DeriveInput) -> TokenStream {
    let DeriveInput {
        ident,
        generics,
        data,
        ..
    } = input;

    let lifetime_generics = add_js_lifetime(&generics);
    let crate_name = format_ident!("{}", crate_ident());

    match data {
        Data::Struct(struct_) => {
            let DataStruct { fields, .. } = struct_;

            let trace_body = match Fields::from_fields(fields) {
                Fields::Named(mut f) => {
                    let fields = f.iter_mut().map(|x| x.expand_trace_body_named(&crate_name));
                    quote!(
                        #(#fields)*
                    )
                }
                Fields::Unnamed(mut f) => {
                    let fields = f.iter_mut().enumerate().map(|(idx, x)| {
                        x.expand_trace_body_unnamed(&crate_name, idx.try_into().unwrap())
                    });
                    quote!(
                        #(#fields)*
                    )
                }
                Fields::Unit => TokenStream::new(),
            };

            quote! {
                impl #lifetime_generics #crate_name::class::Trace<'js> for #ident #generics{
                    fn trace<'a>(&self, _tracer: #crate_name::class::Tracer<'a,'js>){
                        #trace_body
                    }
                }
            }
        }
        Data::Enum(DataEnum { variants, .. }) => {
            let body = variants.into_iter().map(|x| {
                let ident = &x.ident;
                let fields = Fields::from_fields(x.fields);
                match fields {
                    Fields::Named(f) => {
                        let has_skip = f.iter().any(|x| x.config.skip_trace);
                        let field_names = f
                            .iter()
                            .filter_map(|f| (!f.config.skip_trace).then_some(&f.ident))
                            .collect::<Vec<_>>();

                        let remainder = has_skip.then_some(quote!(..));

                        let pattern = quote! {
                            Self::#ident{ #(ref #field_names,)* #remainder }
                        };

                        let body = quote! {
                            {
                                #(#crate_name::class::Trace::trace(#field_names, _tracer);)*
                            }
                        };

                        quote! {
                            #pattern => #body
                        }
                    }
                    Fields::Unnamed(f) => {
                        let patterns = f.iter().enumerate().map(|(idx, f)| {
                            if f.config.skip_trace {
                                quote!(_)
                            } else {
                                let ident = format_ident!("tmp_{idx}");
                                quote!(ref #ident)
                            }
                        });
                        let pattern = quote!(Self::#ident(#(#patterns),*));

                        let names = f.iter().enumerate().filter_map(|(idx, f)| {
                            (!f.config.skip_trace).then(|| {
                                let ident = format_ident!("tmp_{idx}");
                                Some(ident)
                            })
                        });
                        let body = quote! {
                            {
                                #(#crate_name::class::Trace::trace(#names,_tracer);)*
                            }
                        };

                        quote! {
                            #pattern => #body
                        }
                    }
                    Fields::Unit => {
                        quote! {
                            Self::#ident => {}
                        }
                    }
                }
            });

            quote! {
                impl #lifetime_generics #crate_name::class::Trace<'js> for #ident #generics {
                    fn trace<'a>(&self, _tracer: #crate_name::class::Tracer<'a,'js>){
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
