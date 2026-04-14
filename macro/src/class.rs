use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote};
use syn::{
    fold::Fold,
    parse::{Parse, ParseStream},
    punctuated::{Pair, Punctuated},
    spanned::Spanned,
    Error, ItemEnum, ItemStruct, LitStr, Result, Token,
};

use crate::{
    attrs::{take_attributes, FlagOption, OptionList, ValueOption},
    common::{add_js_lifetime, crate_ident, kw, Case},
    fields::Fields,
};

#[derive(Debug, Default, Clone)]
pub(crate) struct ClassConfig {
    pub frozen: bool,
    pub exotic: bool,
    pub crate_: Option<String>,
    pub rename: Option<String>,
    pub rename_all: Option<Case>,
}

pub(crate) enum ClassOption {
    Frozen(FlagOption<kw::frozen>),
    Exotic(FlagOption<kw::exotic>),
    Crate(ValueOption<Token![crate], LitStr>),
    Rename(ValueOption<kw::rename, LitStr>),
    RenameAll(ValueOption<kw::rename_all, Case>),
}

impl Parse for ClassOption {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.peek(kw::frozen) {
            input.parse().map(Self::Frozen)
        } else if input.peek(kw::exotic) {
            input.parse().map(Self::Exotic)
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
            ClassOption::Exotic(ref x) => {
                self.exotic = x.is_true();
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

    pub fn crate_name(&self) -> Result<String> {
        if let Some(c) = self.crate_.clone() {
            return Ok(c);
        }
        crate_ident()
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

struct ErrorAttribute(Result<()>);

impl Fold for ErrorAttribute {
    fn fold_attribute(&mut self, i: syn::Attribute) -> syn::Attribute {
        if self.0.is_err() {
            return i;
        }

        if i.path().is_ident("qjs") {
            self.0 = Err(Error::new(i.span(), "qjs attributes not supported here"))
        }
        i
    }
}

impl Class {
    pub fn from_proc_macro_input(
        options: OptionList<ClassOption>,
        item: syn::Item,
    ) -> Result<Self> {
        let mut config = ClassConfig::default();
        options.0.iter().for_each(|x| config.apply(x));

        match item {
            syn::Item::Enum(enum_) => Self::from_enum(config, enum_),
            syn::Item::Struct(struct_) => Self::from_struct(config, struct_),
            x => Err(Error::new(
                x.span(),
                "class macro can only be applied to enum's and struct",
            )),
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

    pub fn attrs(&self) -> &[syn::Attribute] {
        match self {
            Class::Struct { ref attrs, .. } => attrs,
            Class::Enum { ref attrs, .. } => attrs,
        }
    }

    pub fn from_enum(mut config: ClassConfig, enum_: ItemEnum) -> Result<Self> {
        let ItemEnum {
            mut attrs,
            vis,
            enum_token,
            ident,
            generics,
            variants,
            ..
        } = enum_;

        let mut new_variants = Punctuated::new();
        for variant in variants.into_pairs() {
            match variant {
                Pair::Punctuated(v, c) => {
                    let mut ensure_valid = ErrorAttribute(Ok(()));
                    let v = ensure_valid.fold_variant(v);
                    ensure_valid.0?;

                    new_variants.push(v);
                    new_variants.push_punct(c);
                }
                Pair::End(v) => {
                    let mut ensure_valid = ErrorAttribute(Ok(()));
                    let v = ensure_valid.fold_variant(v);
                    ensure_valid.0?;

                    new_variants.push(v);
                }
            }
        }
        let variants = new_variants;

        take_attributes(&mut attrs, |attr| {
            if !attr.path().is_ident("qjs") {
                return Ok(false);
            }

            let options: OptionList<ClassOption> = attr.parse_args()?;
            options.0.iter().for_each(|x| {
                config.apply(x);
            });
            Ok(true)
        })?;

        Ok(Class::Enum {
            config,
            attrs,
            vis,
            enum_token,
            ident,
            generics,
            variants,
        })
    }

    pub fn from_struct(mut config: ClassConfig, struct_: ItemStruct) -> Result<Self> {
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
        })?;

        let fields = Fields::from_fields(fields)?;

        Ok(Class::Struct {
            config,
            attrs,
            vis,
            struct_token,
            ident,
            generics,
            fields,
        })
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

    pub fn expand(self) -> Result<TokenStream> {
        ensure_no_conflicting_derives(self.attrs())?;

        let crate_name = format_ident!("{}", self.config().crate_name()?);
        let class_name = self.ident().clone();
        let javascript_name = self.javascript_name();
        let module_name = format_ident!("__impl_class_{}_", self.ident());

        let generics = self.generics().clone();
        let generics_with_lifetimes = add_js_lifetime(&generics);

        let mutability = self.mutability();
        let props = self.expand_props(&crate_name);
        let reexpand = self.reexpand();
        let exotic_const = if self.config().exotic {
            quote! { const KIND: #crate_name::class::ClassKind = #crate_name::class::ClassKind::Exotic; }
        } else {
            TokenStream::new()
        };

        let exotic_methods = if self.config().exotic {
            let exotic_module = format_ident!("__impl_exotic_{}__", self.ident());
            quote! {
                fn exotic_get_property(
                    this: &#crate_name::class::JsCell<'js, Self>,
                    ctx: &#crate_name::Ctx<'js>,
                    atom: #crate_name::Atom<'js>,
                    receiver: #crate_name::Value<'js>,
                ) -> #crate_name::Result<#crate_name::Value<'js>> {
                    #exotic_module::ExoticImpl::exotic_get_property(this, ctx, atom, receiver)
                }

                fn exotic_set_property(
                    this: &#crate_name::class::JsCell<'js, Self>,
                    ctx: &#crate_name::Ctx<'js>,
                    atom: #crate_name::Atom<'js>,
                    receiver: #crate_name::Value<'js>,
                    value: #crate_name::Value<'js>,
                ) -> #crate_name::Result<bool> {
                    #exotic_module::ExoticImpl::exotic_set_property(this, ctx, atom, receiver, value)
                }

                fn exotic_delete_property(
                    this: &#crate_name::class::JsCell<'js, Self>,
                    ctx: &#crate_name::Ctx<'js>,
                    atom: #crate_name::Atom<'js>,
                ) -> #crate_name::Result<bool> {
                    #exotic_module::ExoticImpl::exotic_delete_property(this, ctx, atom)
                }

                fn exotic_has_property(
                    this: &#crate_name::class::JsCell<'js, Self>,
                    ctx: &#crate_name::Ctx<'js>,
                    atom: #crate_name::Atom<'js>,
                ) -> #crate_name::Result<bool> {
                    #exotic_module::ExoticImpl::exotic_has_property(this, ctx, atom)
                }
            }
        } else {
            TokenStream::new()
        };

        let res = quote! {
            #reexpand

            #[allow(non_snake_case)]
            mod #module_name{
                pub use super::*;

                impl #generics_with_lifetimes #crate_name::class::JsClass<'js> for #class_name #generics{
                    const NAME: &'static str = #javascript_name;

                    type Mutable = #crate_name::class::#mutability;

                    #exotic_const

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

                    #exotic_methods
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
        };

        Ok(res)
    }
}

pub(crate) fn expand(options: OptionList<ClassOption>, item: syn::Item) -> Result<TokenStream> {
    Class::from_proc_macro_input(options, item)?.expand()
}

/// Reports a helpful compile error when the class' `#[derive(...)]` list
/// includes `FromJs` or `IntoJs`. `#[class]` already generates those impls
/// and deriving them in addition would produce an unhelpful `E0119`
/// conflicting-implementations error pointing inside the generated
/// `__impl_class_*` module. The two macros are conceptually incompatible:
/// `#[class]` round-trips through a `Class<Self>` JS instance, whereas the
/// derives round-trip through a plain object/array, so we simply forbid
/// stacking them.
fn ensure_no_conflicting_derives(attrs: &[syn::Attribute]) -> Result<()> {
    for attr in attrs {
        if !attr.path().is_ident("derive") {
            continue;
        }
        let mut conflict: Option<(String, proc_macro2::Span)> = None;
        attr.parse_nested_meta(|meta| {
            if conflict.is_some() {
                return Ok(());
            }
            if let Some(last) = meta.path.segments.last() {
                let name = last.ident.to_string();
                if name == "FromJs" || name == "IntoJs" {
                    conflict = Some((name, last.ident.span()));
                }
            }
            Ok(())
        })?;
        if let Some((name, span)) = conflict {
            return Err(Error::new(
                span,
                format!(
                    "`#[rquickjs::class]` already implements `{name}` for this type; \
                     remove `{name}` from `#[derive(...)]`, or drop `#[rquickjs::class]` \
                     if you want plain-data conversion"
                ),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use super::ensure_no_conflicting_derives;
    use quote::quote;

    fn attrs_of(input: proc_macro2::TokenStream) -> Vec<syn::Attribute> {
        syn::parse2::<syn::ItemStruct>(input).unwrap().attrs
    }

    #[test]
    fn accepts_empty_attrs() {
        let attrs = attrs_of(quote! {
            struct Foo { x: u32 }
        });
        ensure_no_conflicting_derives(&attrs).expect("no derive is fine");
    }

    #[test]
    fn accepts_unrelated_derives() {
        let attrs = attrs_of(quote! {
            #[derive(Clone, Debug, PartialEq, Eq)]
            struct Foo { x: u32 }
        });
        ensure_no_conflicting_derives(&attrs).expect("unrelated derives are fine");
    }

    #[test]
    fn rejects_derive_from_js() {
        let attrs = attrs_of(quote! {
            #[derive(FromJs)]
            struct Foo { x: u32 }
        });
        let err = ensure_no_conflicting_derives(&attrs).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("`FromJs`"), "unexpected message: {msg}");
        assert!(
            msg.contains("already implements"),
            "unexpected message: {msg}"
        );
    }

    #[test]
    fn rejects_derive_into_js() {
        let attrs = attrs_of(quote! {
            #[derive(IntoJs)]
            struct Foo { x: u32 }
        });
        let err = ensure_no_conflicting_derives(&attrs).unwrap_err();
        assert!(err.to_string().contains("`IntoJs`"));
    }

    #[test]
    fn rejects_mixed_derive_list() {
        // The conflicting derive can appear anywhere in the list, next
        // to arbitrary unrelated derives.
        let attrs = attrs_of(quote! {
            #[derive(Clone, Trace, JsLifetime, FromJs, IntoJs)]
            struct Foo { x: u32 }
        });
        let err = ensure_no_conflicting_derives(&attrs).unwrap_err();
        // We stop at the first match, which is `FromJs`.
        assert!(err.to_string().contains("`FromJs`"));
    }

    #[test]
    fn rejects_path_qualified_derive() {
        // Users sometimes write `#[derive(rquickjs::FromJs)]`.
        let attrs = attrs_of(quote! {
            #[derive(rquickjs::FromJs)]
            struct Foo { x: u32 }
        });
        let err = ensure_no_conflicting_derives(&attrs).unwrap_err();
        assert!(err.to_string().contains("`FromJs`"));
    }

    #[test]
    fn accepts_multiple_derive_attributes() {
        // Multiple `#[derive(...)]` attributes on the same item are legal
        // in Rust; none of them should trip the check if they're unrelated.
        let attrs = attrs_of(quote! {
            #[derive(Clone)]
            #[derive(Debug)]
            struct Foo { x: u32 }
        });
        ensure_no_conflicting_derives(&attrs).expect("unrelated derives are fine");
    }
}
