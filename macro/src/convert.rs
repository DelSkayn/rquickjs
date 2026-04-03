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

pub(crate) fn expand_from_js(mut input: DeriveInput) -> Result<TokenStream> {
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
