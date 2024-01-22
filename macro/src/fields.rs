use convert_case::Casing;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{
    parse::{Parse, ParseStream},
    Attribute, Ident, LitStr, Type, Visibility,
};

use crate::{
    attrs::{take_attributes, FlagOption, OptionList, ValueOption},
    common::{kw, AbortResultExt, Case},
};

#[derive(Default, Debug)]
pub struct FieldConfig {
    pub get: bool,
    pub set: bool,
    pub enumerable: bool,
    pub configurable: bool,
    pub skip_trace: bool,
    pub rename: Option<String>,
}

#[derive(Debug)]
pub(crate) enum FieldOption {
    Get(FlagOption<kw::get>),
    Set(FlagOption<kw::set>),
    Enumerable(FlagOption<kw::enumerable>),
    Configurable(FlagOption<kw::configurable>),
    SkipTrace(FlagOption<kw::skip_trace>),
    Rename(ValueOption<kw::rename, LitStr>),
}

impl Parse for FieldOption {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.peek(kw::get) {
            input.parse().map(Self::Get)
        } else if input.peek(kw::set) {
            input.parse().map(Self::Set)
        } else if input.peek(kw::enumerable) {
            input.parse().map(Self::Enumerable)
        } else if input.peek(kw::configurable) {
            input.parse().map(Self::Configurable)
        } else if input.peek(kw::skip_trace) {
            input.parse().map(Self::SkipTrace)
        } else if input.peek(kw::rename) {
            input.parse().map(Self::Rename)
        } else {
            Err(syn::Error::new(
                input.span(),
                "invalid class field attribute",
            ))
        }
    }
}

impl FieldConfig {
    pub(crate) fn from_attributes(attrs: &mut Vec<syn::Attribute>) -> Self {
        let mut config = Self::default();

        take_attributes(attrs, |attr| {
            if !attr.path().is_ident("qjs") {
                return Ok(false);
            }

            let separated_options: OptionList<FieldOption> = attr.parse_args()?;
            separated_options.0.iter().for_each(|x| config.apply(x));
            Ok(true)
        })
        .unwrap_or_abort();

        config
    }

    pub(crate) fn apply(&mut self, option: &FieldOption) {
        match option {
            FieldOption::Get(ref x) => {
                self.get = x.is_true();
            }
            FieldOption::Set(ref x) => {
                self.set = x.is_true();
            }
            FieldOption::Enumerable(ref x) => {
                self.enumerable = x.is_true();
            }
            FieldOption::Configurable(ref x) => {
                self.configurable = x.is_true();
            }
            FieldOption::SkipTrace(ref x) => {
                self.skip_trace = x.is_true();
            }
            FieldOption::Rename(ref x) => {
                self.rename = Some(x.value.value());
            }
        }
    }
}

#[derive(Debug)]
pub(crate) enum Fields {
    Named(Vec<Field>),
    Unnamed(Vec<Field>),
    Unit,
}

impl Fields {
    pub fn from_fields(input: syn::Fields) -> Self {
        match input {
            syn::Fields::Named(x) => {
                Fields::Named(x.named.into_iter().map(Field::from_field).collect())
            }
            syn::Fields::Unnamed(x) => {
                Fields::Unnamed(x.unnamed.into_iter().map(Field::from_field).collect())
            }
            syn::Fields::Unit => Self::Unit,
        }
    }
}

#[derive(Debug)]
pub(crate) struct Field {
    pub config: FieldConfig,
    pub attrs: Vec<Attribute>,
    pub vis: Visibility,
    pub ident: Option<Ident>,
    pub ty: Type,
}

impl Field {
    fn from_field(
        syn::Field {
            mut attrs,
            vis,
            ident,
            ty,
            ..
        }: syn::Field,
    ) -> Self {
        let config = FieldConfig::from_attributes(&mut attrs);
        Field {
            config,
            attrs,
            vis,
            ident,
            ty,
        }
    }
}

impl Field {
    pub fn expand_prop_config(&self) -> TokenStream {
        let mut res = TokenStream::new();
        if self.config.configurable {
            res.extend(quote!(.configurable()));
        }
        if self.config.enumerable {
            res.extend(quote!(.enumerable()));
        }
        res
    }

    pub fn expand_trace_body_named(&self, lib_crate: &Ident) -> TokenStream {
        if self.config.skip_trace {
            return TokenStream::new();
        }
        let field = self.ident.as_ref().unwrap();

        quote! {
            #lib_crate::class::Trace::<'js>::trace(&self.#field,_tracer);
        }
    }

    pub fn expand_trace_body_unnamed(&self, crate_name: &Ident, which: u32) -> TokenStream {
        if self.config.skip_trace {
            return TokenStream::new();
        }
        let field = format_ident!("{which}");

        quote! {
            #crate_name::class::Trace::<'js>::trace(&self.#field,_tracer);
        }
    }

    pub fn expand_property_named(&self, crate_name: &Ident, case: Option<Case>) -> TokenStream {
        if !(self.config.get || self.config.set) {
            return TokenStream::new();
        }

        let field = self.ident.as_ref().unwrap();
        let ty = &self.ty;

        let accessor = self.expand_accessor(field, crate_name, ty);
        let prop_config = self.expand_prop_config();
        let name = if let Some(rename) = self.config.rename.clone() {
            rename
        } else if let Some(case) = case {
            field.to_string().to_case(case.to_convert_case())
        } else {
            field.to_string()
        };

        quote! {
            proto.prop(#name, #accessor #prop_config)?;
        }
    }

    pub fn expand_property_unnamed(&self, crate_name: &Ident, name: u32) -> TokenStream {
        if !(self.config.get || self.config.set) {
            return TokenStream::new();
        }

        let field = format_ident!("{}", name);
        let ty = &self.ty;
        let accessor = self.expand_accessor(&field, crate_name, ty);
        let prop_config = self.expand_prop_config();
        let name = if let Some(rename) = self.config.rename.clone() {
            quote!(#rename)
        } else {
            quote!(#name as u32)
        };

        quote! {
            proto.prop(#name, #accessor #prop_config)?;
        }
    }

    pub fn expand_accessor(&self, field: &Ident, crate_name: &Ident, ty: &Type) -> TokenStream {
        if self.config.get && self.config.set {
            quote! {
                #crate_name::object::Accessor::new(
                    |this: #crate_name::function::This<#crate_name::class::OwnedBorrow<'js, Self>>|{
                        this.0.#field.clone()
                    },
                    |mut this: #crate_name::function::This<#crate_name::class::OwnedBorrowMut<'js, Self>>, v: #ty|{
                        this.0.#field = v;
                    }
                )
            }
        } else if self.config.get {
            quote! {
                #crate_name::object::Accessor::new_get(
                    |this: #crate_name::function::This<#crate_name::class::OwnedBorrow<'js, Self>>|{
                        this.0.#field.clone()
                    },
                )
            }
        } else if self.config.set {
            quote! {
                #crate_name::object::Accessor::new_set(
                    |mut this: #crate_name::function::This<#crate_name::class::OwnedBorrowMut<'js, Self>>, v: #ty|{
                        this.0.#field = v;
                    }
                )
            }
        } else {
            panic!("called expand_accessor on non accessor field")
        }
    }

    pub fn expand_attrs(&self) -> TokenStream {
        if self.config.skip_trace {
            quote! {
                #[qjs(skip_trace)]
            }
        } else {
            TokenStream::new()
        }
    }

    pub fn expand_field(&self) -> TokenStream {
        let Field {
            ref ident,
            ref vis,
            ref ty,
            ref attrs,
            ..
        } = self;

        let rexported_attrs = self.expand_attrs();

        if let Some(ref ident) = ident {
            quote! {
                #(#attrs)*
                #rexported_attrs
                #vis #ident: #ty
            }
        } else {
            quote! {
                #(#attrs)*
                #rexported_attrs
                #vis #ty
            }
        }
    }
}
