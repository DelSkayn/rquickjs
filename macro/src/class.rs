use proc_macro2::{Ident, TokenStream};
use proc_macro_error::abort;
use quote::{format_ident, quote};
use syn::{
    fold::Fold,
    parse::{Parse, ParseStream},
    punctuated::Pair,
    ItemEnum, ItemStruct, LitStr, Token,
};

use crate::{
    attrs::{take_attributes, FlagOption, OptionList, ValueOption},
    common::{add_js_lifetime, crate_ident, kw, AbortResultExt, Case},
    fields::Fields,
};

#[derive(Debug, Default, Clone)]
pub(crate) struct ClassConfig {
    pub frozen: bool,
    pub crate_: Option<String>,
    pub rename: Option<String>,
    pub rename_all: Option<Case>,
}

pub(crate) enum ClassOption {
    Frozen(FlagOption<kw::frozen>),
    Crate(ValueOption<Token![crate], LitStr>),
    Rename(ValueOption<kw::rename, LitStr>),
    RenameAll(ValueOption<kw::rename_all, Case>),
}

impl Parse for ClassOption {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.peek(kw::frozen) {
            input.parse().map(Self::Frozen)
        } else if input.peek(Token![crate]) {
            input.parse().map(Self::Crate)
        } else if input.peek(kw::rename) {
            input.parse().map(Self::Rename)
        } else if input.peek(kw::rename_all) {
            input.parse().map(Self::RenameAll)
        } else {
            Err(syn::Error::new(input.span(), "invalid class attribute"))
        }
    }
}

impl ClassConfig {
    pub fn apply(&mut self, option: &ClassOption) {
        match option {
            ClassOption::Frozen(ref x) => {
                self.frozen = x.is_true();
            }
            ClassOption::Crate(ref x) => {
                self.crate_ = Some(x.value.value());
            }
            ClassOption::Rename(ref x) => {
                self.rename = Some(x.value.value());
            }
            ClassOption::RenameAll(ref x) => {
                self.rename_all = Some(x.value);
            }
        }
    }

    pub fn crate_name(&self) -> String {
        self.crate_.clone().unwrap_or_else(crate_ident)
    }
}

#[derive(Debug)]
pub(crate) enum Class {
    Enum {
        config: ClassConfig,
        attrs: Vec<syn::Attribute>,
        vis: syn::Visibility,
        enum_token: Token![enum],
        ident: Ident,
        generics: syn::Generics,
        variants: syn::punctuated::Punctuated<syn::Variant, Token![,]>,
    },
    Struct {
        config: ClassConfig,
        attrs: Vec<syn::Attribute>,
        vis: syn::Visibility,
        struct_token: Token![struct],
        ident: Ident,
        generics: syn::Generics,
        fields: Fields,
    },
}

struct ErrorAttribute;

impl Fold for ErrorAttribute {
    fn fold_attribute(&mut self, i: syn::Attribute) -> syn::Attribute {
        if i.path().is_ident("qjs") {
            abort!(i, "qjs attributes not supported here")
        }
        i
    }
}

impl Class {
    pub fn from_proc_macro_input(options: OptionList<ClassOption>, item: syn::Item) -> Self {
        let mut config = ClassConfig::default();
        options.0.iter().for_each(|x| config.apply(x));

        match item {
            syn::Item::Enum(enum_) => Self::from_enum(config, enum_),
            syn::Item::Struct(struct_) => Self::from_struct(config, struct_),
            x => abort!(x, "class macro can only be applied to enum's and structs"),
        }
    }

    pub fn config(&self) -> &ClassConfig {
        match self {
            Class::Enum { ref config, .. } => config,
            Class::Struct { ref config, .. } => config,
        }
    }

    pub fn ident(&self) -> &Ident {
        match self {
            Class::Struct { ref ident, .. } => ident,
            Class::Enum { ref ident, .. } => ident,
        }
    }

    pub fn from_enum(mut config: ClassConfig, enum_: ItemEnum) -> Self {
        let ItemEnum {
            mut attrs,
            vis,
            enum_token,
            ident,
            generics,
            variants,
            ..
        } = enum_;

        let variants = variants
            .into_pairs()
            .map(|x| match x {
                Pair::Punctuated(v, c) => Pair::Punctuated(ErrorAttribute.fold_variant(v), c),
                Pair::End(v) => Pair::End(ErrorAttribute.fold_variant(v)),
            })
            .collect();

        take_attributes(&mut attrs, |attr| {
            if !attr.path().is_ident("qjs") {
                return Ok(false);
            }

            let options: OptionList<ClassOption> = attr.parse_args()?;
            options.0.iter().for_each(|x| {
                config.apply(x);
            });
            Ok(true)
        })
        .unwrap_or_abort();

        Class::Enum {
            config,
            attrs,
            vis,
            enum_token,
            ident,
            generics,
            variants,
        }
    }

    pub fn from_struct(mut config: ClassConfig, struct_: ItemStruct) -> Self {
        let ItemStruct {
            mut attrs,
            vis,
            struct_token,
            ident,
            generics,
            fields,
            ..
        } = struct_;

        take_attributes(&mut attrs, |attr| {
            if !attr.path().is_ident("qjs") {
                return Ok(false);
            }

            let options: OptionList<ClassOption> = attr.parse_args()?;
            options.0.iter().for_each(|x| {
                config.apply(x);
            });
            Ok(true)
        })
        .unwrap_or_abort();

        let fields = Fields::from_fields(fields);

        Class::Struct {
            config,
            attrs,
            vis,
            struct_token,
            ident,
            generics,
            fields,
        }
    }

    pub fn generics(&self) -> &syn::Generics {
        match self {
            Class::Enum { ref generics, .. } => generics,
            Class::Struct { ref generics, .. } => generics,
        }
    }

    pub fn javascript_name(&self) -> String {
        self.config()
            .rename
            .clone()
            .unwrap_or_else(|| self.ident().to_string())
    }

    pub fn mutability(&self) -> TokenStream {
        if self.config().frozen {
            quote! {
               Readable
            }
        } else {
            quote! {
                Writable
            }
        }
    }

    pub fn expand_props(&self, crate_name: &Ident) -> TokenStream {
        let Class::Struct { ref fields, .. } = self else {
            return TokenStream::new();
        };

        match fields {
            Fields::Named(x) => {
                let props = x
                    .iter()
                    .map(|x| x.expand_property_named(crate_name, self.config().rename_all));
                quote!(#(#props)*)
            }
            Fields::Unnamed(x) => {
                let props = x
                    .iter()
                    .enumerate()
                    .map(|(idx, x)| x.expand_property_unnamed(crate_name, idx.try_into().unwrap()));
                quote!(#(#props)*)
            }
            Fields::Unit => TokenStream::new(),
        }
    }

    // Aeexpand the original definition with the attributes removed..
    pub fn reexpand(&self) -> TokenStream {
        match self {
            Class::Enum {
                attrs,
                vis,
                enum_token,
                ident,
                generics,
                variants,
                ..
            } => {
                quote! {
                    #(#attrs)*
                    #vis #enum_token #ident #generics { #variants }
                }
            }
            Class::Struct {
                attrs,
                vis,
                struct_token,
                ident,
                generics,
                fields,
                ..
            } => {
                let fields = match fields {
                    Fields::Named(fields) => {
                        let fields = fields.iter().map(|x| x.expand_field());
                        quote! {
                            {
                            #(#fields),*
                            }
                        }
                    }
                    Fields::Unnamed(fields) => {
                        let fields = fields.iter().map(|x| x.expand_field());
                        quote! {
                            (#(#fields),*)
                        }
                    }
                    Fields::Unit => TokenStream::new(),
                };

                quote! {
                    #(#attrs)*
                    #vis #struct_token #ident #generics #fields
                }
            }
        }
    }

    pub fn expand(self) -> TokenStream {
        let crate_name = format_ident!("{}", self.config().crate_name());
        let class_name = self.ident().clone();
        let javascript_name = self.javascript_name();
        let module_name = format_ident!("__impl_class_{}_", self.ident());

        let generics = self.generics().clone();
        let generics_with_lifetimes = add_js_lifetime(&generics);

        let mutability = self.mutability();
        let props = self.expand_props(&crate_name);
        let reexpand = self.reexpand();

        quote! {
            #reexpand

            #[allow(non_snake_case)]
            mod #module_name{
                pub use super::*;

                impl #generics_with_lifetimes #crate_name::class::JsClass<'js> for #class_name #generics{
                    const NAME: &'static str = #javascript_name;

                    type Mutable = #crate_name::class::#mutability;

                    fn class_id() -> &'static #crate_name::class::ClassId{
                        static ID: #crate_name::class::ClassId =  #crate_name::class::ClassId::new();
                        &ID
                    }

                    fn prototype(ctx: &#crate_name::Ctx<'js>) -> #crate_name::Result<Option<#crate_name::Object<'js>>>{
                        use #crate_name::class::impl_::MethodImplementor;

                        let proto = #crate_name::Object::new(ctx.clone())?;
                        #props
                        let implementor = #crate_name::class::impl_::MethodImpl::<Self>::new();
                        (&implementor).implement(&proto)?;
                        Ok(Some(proto))
                    }

                    fn constructor(ctx: &#crate_name::Ctx<'js>) -> #crate_name::Result<Option<#crate_name::function::Constructor<'js>>>{
                        use #crate_name::class::impl_::ConstructorCreator;

                        let implementor = #crate_name::class::impl_::ConstructorCreate::<Self>::new();
                        (&implementor).create_constructor(ctx)
                    }
                }

                impl #generics_with_lifetimes #crate_name::IntoJs<'js> for #class_name #generics{
                    fn into_js(self,ctx: &#crate_name::Ctx<'js>) -> #crate_name::Result<#crate_name::Value<'js>>{
                        let cls = #crate_name::class::Class::<Self>::instance(ctx.clone(),self)?;
                        #crate_name::IntoJs::into_js(cls, ctx)
                    }
                }

                impl #generics_with_lifetimes #crate_name::FromJs<'js> for #class_name #generics
                where
                    for<'a> #crate_name::class::impl_::CloneWrapper<'a,Self>: #crate_name::class::impl_::CloneTrait<Self>,
                {
                    fn from_js(ctx: &#crate_name::Ctx<'js>, value: #crate_name::Value<'js>) -> #crate_name::Result<Self>{
                        use #crate_name::class::impl_::CloneTrait;

                        let value = #crate_name::class::Class::<Self>::from_js(ctx,value)?;
                        let borrow = value.try_borrow()?;
                        Ok(#crate_name::class::impl_::CloneWrapper(&*borrow).wrap_clone())
                    }
                }
            }
        }
    }
}

pub(crate) fn expand(options: OptionList<ClassOption>, item: syn::Item) -> TokenStream {
    Class::from_proc_macro_input(options, item).expand()
}
