use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{
    spanned::Spanned as _, visit::Visit, Data, DeriveInput, Error, Field, Fields, GenericArgument,
    GenericParam, Generics, Lifetime, Result, Type,
};

use crate::{
    attrs::{take_attributes, OptionList},
    common::crate_ident,
    trace::{ImplConfig, TraceOption},
};

pub fn retrieve_lifetime(generics: &Generics) -> Result<Option<&Lifetime>> {
    let mut lifetime: Option<&Lifetime> = None;
    for p in generics.params.iter() {
        match p {
            GenericParam::Lifetime(x) => {
                if let Some(x) = lifetime.as_ref() {
                    return Err(Error::new(x.span(),"Type has multiple lifetimes, this is not supported by the JsLifetime derive macro"));
                }
                lifetime = Some(&x.lifetime);
            }
            _ => {}
        }
    }

    Ok(lifetime)
}

pub fn extract_types_need_checking(lt: &Lifetime, data: &Data) -> Result<Vec<Type>> {
    let mut res = Vec::new();

    match data {
        Data::Struct(s) => {
            for f in s.fields.iter() {
                extract_types_need_checking_fields(lt, &f, &mut res)?;
            }
        }
        Data::Enum(e) => {
            for v in e.variants.iter() {
                let fields = match v.fields {
                    Fields::Unit => continue,
                    Fields::Named(ref x) => &x.named,
                    Fields::Unnamed(ref x) => &x.unnamed,
                };

                for f in fields {
                    extract_types_need_checking_fields(lt, &f, &mut res)?;
                }
            }
        }
        Data::Union(u) => {
            return Err(Error::new(
                u.union_token.span(),
                "Union types are not supported",
            ))
        }
    }

    Ok(res)
}

pub struct LtTypeVisitor<'a>(Result<bool>, &'a Lifetime);

impl<'ast> Visit<'ast> for LtTypeVisitor<'ast> {
    fn visit_generic_argument(&mut self, i: &'ast syn::GenericArgument) {
        if self.0.is_err() {
            return;
        }

        match i {
            GenericArgument::Lifetime(lt) => {
                if lt.ident == "static" || lt == self.1 {
                    self.0 = Ok(true)
                } else {
                    self.0 = Err(Error::new(
                        lt.span(),
                        "Type contained lifetime which was not static or the 'js lifetime",
                    ));
                }
            }
            _ => {}
        }

        syn::visit::visit_generic_argument(self, i);
    }

    fn visit_type(&mut self, i: &'ast syn::Type) {
        if self.0.is_err() {
            return;
        }

        syn::visit::visit_type(self, i)
    }
}

pub fn extract_types_need_checking_fields(
    lt: &Lifetime,
    field: &Field,
    types: &mut Vec<Type>,
) -> Result<()> {
    let mut visitor = LtTypeVisitor(Ok(false), lt);
    visitor.visit_type(&field.ty);

    if visitor.0? {
        types.push(field.ty.clone());
    }
    Ok(())
}

pub fn extract_bounds(generics: &Generics) -> Result<Vec<TokenStream>> {
    let mut res = Vec::new();

    for p in generics.params.iter() {
        match p {
            GenericParam::Lifetime(_) => {}
            GenericParam::Type(x) => res.push(quote! {
                #x: JsLifetime<'js>
            }),
            GenericParam::Const(_) => {}
        }
    }

    Ok(res)
}

pub(crate) fn expand(mut input: DeriveInput) -> Result<TokenStream> {
    let name = input.ident;

    let mut config = ImplConfig::default();
    take_attributes(&mut input.attrs, |attr| {
        if !attr.path().is_ident("qjs") {
            return Ok(false);
        }

        let options: OptionList<TraceOption> = attr.parse_args()?;
        options.0.iter().for_each(|x| config.apply(x));
        Ok(true)
    })?;

    let crate_name = if let Some(x) = config.crate_.clone() {
        format_ident!("{x}")
    } else {
        format_ident!("{}", crate_ident()?)
    };

    let generics = &input.generics;
    let lt = retrieve_lifetime(generics)?;
    let bounds = extract_bounds(generics)?;

    let Some(lt) = lt else {
        let res = quote! {
            unsafe impl<'js> #crate_name::JsLifetime<'js> for #name #generics
                where #(#bounds),*
            {
                type Changed<'to> = #name;
            }
        };
        return Ok(res);
    };

    let types = extract_types_need_checking(&lt, &input.data)?;

    let const_name = format_ident!("__{}__LT_TYPE_CHECK", name.to_string().to_uppercase());

    let res = quote! {
        const #const_name: () = const {
            trait ValidJsLifetimeImpl{};

            impl<#lt> ValidJsLifetimeImpl for #name<#lt>
                where
                    #(
                        #types: JsLifetime<#lt>
                    ),*
            {
            }

            const fn assert_js_lifetime_impl_is_valid<T: ValidJsLifetimeImpl>(){}

            assert_js_lifetime_impl_is_valid::<#name>()
        };

        unsafe impl<#lt> #crate_name::JsLifetime<#lt> for #name #generics
            where #(#bounds),*
        {
            type Changed<'to> = #name<'to>;
        }
    };
    Ok(res)
}
