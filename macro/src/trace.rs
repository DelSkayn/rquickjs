use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{
    parse::{Parse, ParseStream},
    spanned::Spanned,
    Data, DataEnum, DataStruct, DeriveInput, Error, LitStr, Result, Token,
};

use crate::{
    attrs::{take_attributes, OptionList, ValueOption},
    common::{add_js_lifetime, crate_ident},
    fields::Fields,
};

#[derive(Default)]
pub(crate) struct ImplConfig {
    pub(crate) crate_: Option<String>,
}

impl ImplConfig {
    pub fn apply(&mut self, option: &TraceOption) {
        match option {
            TraceOption::Crate(x) => {
                self.crate_ = Some(x.value.value());
            }
        }
    }
}

pub(crate) enum TraceOption {
    Crate(ValueOption<Token![crate], LitStr>),
}

impl Parse for TraceOption {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.peek(Token![crate]) {
            input.parse().map(Self::Crate)
        } else {
            Err(syn::Error::new(input.span(), "invalid impl attribute"))
        }
    }
}

pub(crate) fn expand(input: DeriveInput) -> Result<TokenStream> {
    let DeriveInput {
        ident,
        generics,
        data,
        mut attrs,
        ..
    } = input;

    let mut config = ImplConfig::default();

    take_attributes(&mut attrs, |attr| {
        if !attr.path().is_ident("qjs") {
            return Ok(false);
        }

        let options: OptionList<TraceOption> = attr.parse_args()?;
        options.0.iter().for_each(|x| config.apply(x));
        Ok(true)
    })?;

    //options.0.iter().for_each(|x| config.apply(x));

    let lifetime_generics = add_js_lifetime(&generics);
    let crate_name = if let Some(x) = config.crate_ {
        format_ident!("{x}")
    } else {
        format_ident!("{}", crate_ident()?)
    };

    match data {
        Data::Struct(struct_) => {
            let DataStruct { fields, .. } = struct_;

            let trace_body = match Fields::from_fields(fields)? {
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

            Ok(quote! {
                impl #lifetime_generics #crate_name::class::Trace<'js> for #ident #generics{
                    fn trace<'a>(&self, _tracer: #crate_name::class::Tracer<'a,'js>){
                        #trace_body
                    }
                }
            })
        }
        Data::Enum(DataEnum { variants, .. }) => {
            let mut body = TokenStream::new();

            for x in variants.into_iter() {
                let ident = &x.ident;
                let fields = Fields::from_fields(x.fields)?;
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

                        let block = quote! {
                            {
                                #(#crate_name::class::Trace::trace(#field_names, _tracer);)*
                            }
                        };

                        body.extend(quote! {
                            #pattern => #block,
                        });
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

                        let names = f
                            .iter()
                            .enumerate()
                            .filter(|&(_idx, f)| !f.config.skip_trace)
                            .map(|(idx, _f)| {
                                let ident = format_ident!("tmp_{idx}");
                                Some(ident)
                            });

                        let block = quote! {
                            {
                                #(#crate_name::class::Trace::trace(#names,_tracer);)*
                            }
                        };

                        body.extend(quote! {
                            #pattern => #block,
                        });
                    }
                    Fields::Unit => body.extend(quote! {
                        Self::#ident => {},
                    }),
                }
            }

            Ok(quote! {
                impl #lifetime_generics #crate_name::class::Trace<'js> for #ident #generics {
                    fn trace<'a>(&self, _tracer: #crate_name::class::Tracer<'a,'js>){
                        match *self{
                            #body
                        }
                    }
                }
            })
        }
        Data::Union(u) => Err(Error::new(
            u.union_token.span(),
            "deriving trace for unions is not supported",
        )),
    }
}
