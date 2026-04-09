use convert_case::Casing;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{
    parse::{Parse, ParseStream},
    spanned::Spanned,
    Data, DataStruct, DeriveInput, Error, Ident, LitStr, Result, Token,
};

use crate::{
    attrs::{take_attributes, OptionList, ValueOption},
    common::{add_js_lifetime, crate_ident, kw, Case},
    fields::{Field, Fields},
};

#[derive(Debug, Default, Clone)]
struct ConvertConfig {
    crate_: Option<String>,
    rename_all: Option<Case>,
}

enum ConvertOption {
    Crate(ValueOption<Token![crate], LitStr>),
    RenameAll(ValueOption<kw::rename_all, Case>),
}

impl Parse for ConvertOption {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.peek(Token![crate]) {
            input.parse().map(Self::Crate)
        } else if input.peek(kw::rename_all) {
            input.parse().map(Self::RenameAll)
        } else {
            Err(Error::new(input.span(), "invalid conversion attribute"))
        }
    }
}

impl ConvertConfig {
    fn apply(&mut self, option: &ConvertOption) {
        match option {
            ConvertOption::Crate(x) => {
                self.crate_ = Some(x.value.value());
            }
            ConvertOption::RenameAll(x) => {
                self.rename_all = Some(x.value);
            }
        }
    }

    fn crate_name(&self) -> Result<String> {
        self.crate_.clone().map(Ok).unwrap_or_else(crate_ident)
    }
}

/// Reports a targeted compile error when the struct we're deriving `FromJs`
/// or `IntoJs` for is also tagged with `#[rquickjs::class]`. `#[class]`
/// already emits its own `FromJs`/`IntoJs` pair that round-trips through a
/// `Class<Self>` instance, whereas the derives round-trip through a plain
/// JS object/array, so combining them produces a conflicting-impl E0119.
/// We catch that here — while the `class` attribute is still attached and
/// unexpanded — so the user sees a message they can act on.
fn ensure_not_on_class(attrs: &[syn::Attribute], trait_name: &str) -> Result<()> {
    for attr in attrs {
        let path = attr.path();
        let Some(last) = path.segments.last() else {
            continue;
        };
        if last.ident != "class" {
            continue;
        }
        let is_ours =
            path.segments.len() == 1 || path.segments.iter().any(|s| s.ident == "rquickjs");
        if !is_ours {
            continue;
        }
        return Err(Error::new(
            last.ident.span(),
            format!(
                "`#[rquickjs::class]` already implements `{trait_name}` for this type; \
                 remove `{trait_name}` from `#[derive(...)]`, or drop `#[rquickjs::class]` \
                 if you want plain-data conversion"
            ),
        ));
    }
    Ok(())
}

pub(crate) fn expand_from_js(mut input: DeriveInput) -> Result<TokenStream> {
    ensure_not_on_class(&input.attrs, "FromJs")?;

    let ident = input.ident;
    let data = input.data;

    let mut config = ConvertConfig::default();
    take_attributes(&mut input.attrs, |attr| {
        if !attr.path().is_ident("qjs") {
            return Ok(false);
        }

        let options: OptionList<ConvertOption> = attr.parse_args()?;
        options.0.iter().for_each(|option| config.apply(option));
        Ok(true)
    })?;

    let crate_name = format_ident!("{}", config.crate_name()?);
    let impl_generics = add_js_lifetime(&input.generics);
    let (impl_generics, _, _) = impl_generics.split_for_impl();
    let (_, ty_generics, where_clause) = input.generics.split_for_impl();

    let body = match data {
        Data::Struct(struct_) => expand_from_js_struct(&crate_name, &ident, &config, struct_)?,
        Data::Enum(enum_) => {
            return Err(Error::new(
                enum_.enum_token.span(),
                "deriving FromJs for enums is not supported yet",
            ));
        }
        Data::Union(union_) => {
            return Err(Error::new(
                union_.union_token.span(),
                "deriving FromJs for unions is not supported",
            ));
        }
    };

    Ok(quote! {
        impl #impl_generics #crate_name::FromJs<'js> for #ident #ty_generics #where_clause {
            fn from_js(_ctx: &#crate_name::Ctx<'js>, value: #crate_name::Value<'js>) -> #crate_name::Result<Self> {
                #body
            }
        }
    })
}

pub(crate) fn expand_into_js(mut input: DeriveInput) -> Result<TokenStream> {
    ensure_not_on_class(&input.attrs, "IntoJs")?;

    let ident = input.ident;
    let data = input.data;

    let mut config = ConvertConfig::default();
    take_attributes(&mut input.attrs, |attr| {
        if !attr.path().is_ident("qjs") {
            return Ok(false);
        }

        let options: OptionList<ConvertOption> = attr.parse_args()?;
        options.0.iter().for_each(|option| config.apply(option));
        Ok(true)
    })?;

    let crate_name = format_ident!("{}", config.crate_name()?);
    let impl_generics = add_js_lifetime(&input.generics);
    let (impl_generics, _, _) = impl_generics.split_for_impl();
    let (_, ty_generics, where_clause) = input.generics.split_for_impl();

    let body = match data {
        Data::Struct(struct_) => expand_into_js_struct(&crate_name, &ident, &config, struct_)?,
        Data::Enum(enum_) => {
            return Err(Error::new(
                enum_.enum_token.span(),
                "deriving IntoJs for enums is not supported yet",
            ));
        }
        Data::Union(union_) => {
            return Err(Error::new(
                union_.union_token.span(),
                "deriving IntoJs for unions is not supported",
            ));
        }
    };

    Ok(quote! {
        impl #impl_generics #crate_name::IntoJs<'js> for #ident #ty_generics #where_clause {
            fn into_js(self, ctx: &#crate_name::Ctx<'js>) -> #crate_name::Result<#crate_name::Value<'js>> {
                #body
            }
        }
    })
}

fn expand_from_js_struct(
    crate_name: &Ident,
    ident: &Ident,
    config: &ConvertConfig,
    struct_: DataStruct,
) -> Result<TokenStream> {
    match Fields::from_fields(struct_.fields)? {
        Fields::Named(fields) => {
            let reads = fields.iter().map(|field| {
                let field_ident = field.ident.as_ref().unwrap();
                let field_name = convert_field_name(field, config.rename_all);
                quote!(#field_ident: value.get(#field_name)?)
            });

            Ok(quote! {
                let value = #crate_name::Object::from_value(value)?;
                Ok(#ident {
                    #(#reads,)*
                })
            })
        }
        Fields::Unnamed(fields) => {
            let reads = fields.iter().enumerate().map(|(index, _)| {
                let index = syn::Index::from(index);
                quote!(value.get(#index)?)
            });

            Ok(quote! {
                let value = #crate_name::Array::from_value(value)?;
                Ok(#ident(
                    #(#reads,)*
                ))
            })
        }
        Fields::Unit => Ok(quote!(Ok(#ident))),
    }
}

fn expand_into_js_struct(
    crate_name: &Ident,
    _ident: &Ident,
    config: &ConvertConfig,
    struct_: DataStruct,
) -> Result<TokenStream> {
    match Fields::from_fields(struct_.fields)? {
        Fields::Named(fields) => {
            let writes = fields.iter().map(|field| {
                let field_ident = field.ident.as_ref().unwrap();
                let field_name = convert_field_name(field, config.rename_all);
                quote!(value.set(#field_name, self.#field_ident)?;)
            });

            Ok(quote! {
                let value = #crate_name::Object::new(ctx.clone())?;
                #(#writes)*
                Ok(value.into_value())
            })
        }
        Fields::Unnamed(fields) => {
            let writes = fields.iter().enumerate().map(|(index, _)| {
                let index_token = syn::Index::from(index);
                quote!(value.set(#index, self.#index_token)?;)
            });

            Ok(quote! {
                let value = #crate_name::Array::new(ctx.clone())?;
                #(#writes)*
                Ok(value.into_value())
            })
        }
        Fields::Unit => Ok(quote!(#crate_name::IntoJs::into_js(#crate_name::Undefined, ctx))),
    }
}

fn convert_field_name(field: &Field, rename_all: Option<Case>) -> String {
    let field_ident = field.ident.as_ref().unwrap();
    field
        .config
        .rename
        .clone()
        .unwrap_or_else(|| match rename_all {
            Some(case) => field_ident.to_string().to_case(case.to_convert_case()),
            None => field_ident.to_string(),
        })
}

#[cfg(test)]
mod test {
    use super::ensure_not_on_class;
    use quote::quote;

    fn attrs_of(input: proc_macro2::TokenStream) -> Vec<syn::Attribute> {
        syn::parse2::<syn::ItemStruct>(input).unwrap().attrs
    }

    #[test]
    fn accepts_struct_without_class_attribute() {
        let attrs = attrs_of(quote! {
            #[derive(Clone, Debug)]
            struct Foo { x: u32 }
        });
        ensure_not_on_class(&attrs, "FromJs").expect("no class attr is fine");
    }

    #[test]
    fn rejects_path_qualified_class() {
        let attrs = attrs_of(quote! {
            #[rquickjs::class]
            struct Foo { x: u32 }
        });
        let err = ensure_not_on_class(&attrs, "FromJs").unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("`FromJs`"), "unexpected message: {msg}");
        assert!(
            msg.contains("already implements"),
            "unexpected message: {msg}"
        );
    }

    #[test]
    fn rejects_class_with_arguments() {
        // `#[rquickjs::class(rename = "Foo")]` is still the same attribute.
        let attrs = attrs_of(quote! {
            #[rquickjs::class(rename = "Foo")]
            struct Foo { x: u32 }
        });
        let err = ensure_not_on_class(&attrs, "IntoJs").unwrap_err();
        assert!(err.to_string().contains("`IntoJs`"));
    }

    #[test]
    fn rejects_bare_class_attribute() {
        // Bare `#[class]` comes up when the user does
        // `use rquickjs::class` and then writes `#[class]`. A one-segment
        // path is treated as ours.
        let attrs = attrs_of(quote! {
            #[class]
            struct Foo { x: u32 }
        });
        ensure_not_on_class(&attrs, "FromJs")
            .expect_err("bare `#[class]` should be detected as ours");
    }

    #[test]
    fn ignores_unrelated_class_attribute() {
        // An attribute whose path ends in `class` but isn't ours
        // (no `rquickjs` segment, more than one segment) should be
        // ignored so we don't produce false positives on downstream
        // attribute macros that happen to be named `class`.
        let attrs = attrs_of(quote! {
            #[some_other_crate::class]
            struct Foo { x: u32 }
        });
        ensure_not_on_class(&attrs, "FromJs").expect("third-party ::class attr should not trigger");
    }

    #[test]
    fn trait_name_is_reflected_in_message() {
        let attrs = attrs_of(quote! {
            #[rquickjs::class]
            struct Foo { x: u32 }
        });
        let err_from = ensure_not_on_class(&attrs, "FromJs").unwrap_err();
        let err_into = ensure_not_on_class(&attrs, "IntoJs").unwrap_err();
        assert!(err_from.to_string().contains("`FromJs`"));
        assert!(err_into.to_string().contains("`IntoJs`"));
        assert!(!err_from.to_string().contains("`IntoJs`"));
        assert!(!err_into.to_string().contains("`FromJs`"));
    }
}
