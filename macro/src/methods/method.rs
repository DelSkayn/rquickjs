use convert_case::Casing;
use proc_macro2::{Ident, Span, TokenStream};
use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    spanned::Spanned,
    Attribute, Block, Error, Expr, ImplItemFn, LitStr, Result, Signature, Token, Type, Visibility,
};

use crate::{
    attrs::{take_attributes, FlagOption, OptionList, ValueOption},
    common::{kw, Case},
    function::JsFunction,
};

#[derive(Default, Clone)]
pub(crate) struct MethodConfig {
    pub constructor: bool,
    pub skip: bool,
    pub r#static: bool,
    pub configurable: bool,
    pub enumerable: bool,
    pub get: bool,
    pub set: bool,
    pub rename: Option<Expr>,
}

impl MethodConfig {
    pub fn apply(&mut self, option: &MethodOption) {
        match option {
            MethodOption::Constructor(x) => {
                self.constructor = x.is_true();
            }
            MethodOption::Static(x) => {
                self.r#static = x.is_true();
            }
            MethodOption::Skip(x) => {
                self.skip = x.is_true();
            }
            MethodOption::Configurable(x) => {
                self.configurable = x.is_true();
            }
            MethodOption::Enumerable(x) => {
                self.enumerable = x.is_true();
            }
            MethodOption::Get(x) => {
                self.get = x.is_true();
            }
            MethodOption::Set(x) => {
                self.set = x.is_true();
            }
            MethodOption::Rename(x) => {
                self.rename = Some(x.value.clone());
            }
        }
    }
}

pub(crate) enum MethodOption {
    Constructor(FlagOption<kw::constructor>),
    Static(FlagOption<Token![static]>),
    Skip(FlagOption<kw::skip>),
    Configurable(FlagOption<kw::configurable>),
    Enumerable(FlagOption<kw::enumerable>),
    Get(FlagOption<kw::get>),
    Set(FlagOption<kw::set>),
    Rename(ValueOption<kw::rename, Expr>),
}

impl Parse for MethodOption {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.peek(kw::constructor) {
            input.parse().map(Self::Constructor)
        } else if input.peek(Token![static]) {
            input.parse().map(Self::Static)
        } else if input.peek(kw::skip) {
            input.parse().map(Self::Skip)
        } else if input.peek(kw::configurable) {
            input.parse().map(Self::Configurable)
        } else if input.peek(kw::enumerable) {
            input.parse().map(Self::Enumerable)
        } else if input.peek(kw::get) {
            input.parse().map(Self::Get)
        } else if input.peek(kw::set) {
            input.parse().map(Self::Set)
        } else if input.peek(kw::rename) {
            input.parse().map(Self::Rename)
        } else {
            Err(syn::Error::new(input.span(), "invalid method attribute"))
        }
    }
}

impl MethodConfig {
    /// Make sure attrs aren't applied in ways they shouldn't be.
    /// Span: The span the error should be attached to.
    pub fn validate(&self, span: Span) -> Result<()> {
        if self.get && self.set {
            return Err(Error::new(
                span,
                "a function can't both be a setter and a getter at the same time.",
            ));
        }

        if self.constructor && self.rename.is_some() {
            return Err(Error::new(span, "Can't rename a constructor"));
        }

        if self.constructor && self.get {
            return Err(Error::new(
                span,
                "a function can't both be a getter and a constructor at the same time.",
            ));
        }

        if self.constructor && self.set {
            return Err(Error::new(
                span,
                "a function can't both be a setter and a constructor at the same time.",
            ));
        }

        if self.configurable && !(self.get || self.set) {
            return Err(Error::new(
                span,
                "configurable can only be set for getters and setters.",
            ));
        }

        if self.enumerable && !(self.get || self.set) {
            return Err(Error::new(
                span,
                "enumerable can only be set for getters and setters.",
            ));
        }
        Ok(())
    }
}

#[derive(Clone)]
pub(crate) struct Method {
    pub config: MethodConfig,
    pub attr_span: Span,
    pub function: JsFunction,
    pub attrs: Vec<Attribute>,
    pub vis: Visibility,
    pub sig: Signature,
    pub block: Block,
}

impl Method {
    pub fn parse_impl_fn(func: ImplItemFn, self_ty: &Type) -> Result<Self> {
        let span = func.span();
        let ImplItemFn {
            mut attrs,
            vis,
            defaultness,
            sig,
            block,
        } = func;

        let mut config = MethodConfig::default();
        let mut attr_span = Span::call_site();

        take_attributes(&mut attrs, |attr| {
            if !attr.path().is_ident("qjs") {
                return Ok(false);
            }

            attr_span = attr.span();
            let option_flags = attr.parse_args::<OptionList<MethodOption>>()?;
            for option in option_flags.0.iter() {
                config.apply(option);
            }
            Ok(true)
        })?;

        config.validate(attr_span)?;

        let attr_span = if attrs.is_empty() {
            span
        } else {
            attrs[0].span()
        };

        if let Some(d) = defaultness {
            return Err(Error::new(d.span(), "specialized fn's are not supported."));
        }

        attrs.retain(|x| !x.path().is_ident("qjs"));

        let function = JsFunction::new(vis.clone(), &sig, Some(self_ty))?;

        Ok(Method {
            config,
            attr_span,
            function,
            attrs,
            vis,
            sig,
            block,
        })
    }

    /// The name on of this method on the JavaScript side.
    pub fn name(&self, case: Option<Case>) -> Expr {
        if let Some(x) = self.config.rename.clone() {
            x
        } else {
            let res = self.function.name.to_string();
            let name = if let Some(case) = case {
                res.to_case(case.to_convert_case())
            } else {
                res
            };
            syn::Expr::Lit(syn::ExprLit {
                attrs: Vec::new(),
                lit: syn::Lit::Str(LitStr::new(&name, Span::call_site())),
            })
        }
    }

    pub fn expand_impl(&self) -> TokenStream {
        let attrs = &self.attrs;
        let vis = &self.vis;
        let sig = &self.sig;
        let block = &self.block;

        quote! {
            #(#attrs)* #vis #sig #block
        }
    }

    pub(crate) fn expand_js_impl(&self, prefix: &str, lib_crate: &Ident) -> TokenStream {
        if self.config.skip {
            return TokenStream::new();
        }
        let carry_type = self.function.expand_carry_type(prefix);
        let impl_ = self.function.expand_to_js_function_impl(prefix, lib_crate);
        let into_js = self.function.expand_into_js_impl(prefix, lib_crate);

        quote! {
            #carry_type

            #impl_

            #into_js
        }
    }

    pub(crate) fn expand_associated_type(
        &self,
        associated_prefix: &str,
        impl_prefix: &str,
    ) -> TokenStream {
        if self.config.skip {
            return TokenStream::new();
        }
        let associated_name = self.function.expand_carry_type_name(associated_prefix);
        let impl_name = self.function.expand_carry_type_name(impl_prefix);
        let vis = &self.vis;

        quote! {
            #vis const #associated_name: #impl_name = #impl_name;
        }
    }

    pub(crate) fn expand_apply_to_object(
        &self,
        prefix: &str,
        self_ty: &Type,
        object_name: &Ident,
        case: Option<Case>,
    ) -> TokenStream {
        if self.config.skip {
            return TokenStream::new();
        }
        let func_name_str = self.name(case);
        let js_func_name = self.function.expand_carry_type_name(prefix);
        quote! {
            #object_name.set(#func_name_str,<#self_ty>::#js_func_name)?;
        }
    }
}
