use convert_case::Casing;
use darling::FromAttributes;
use proc_macro2::{Ident, Span, TokenStream};
use proc_macro_error::abort;
use quote::quote;
use syn::{spanned::Spanned, Attribute, Block, ImplItemFn, Signature, Type, Visibility};

use crate::{common::Case, function::JsFunction};

use super::ImplFnAttr;

#[derive(Debug, Clone)]
pub(crate) struct JsMethod {
    pub attr_span: Span,
    pub parse_attrs: ImplFnAttr,
    pub function: JsFunction,
    pub attrs: Vec<Attribute>,
    pub vis: Visibility,
    pub sig: Signature,
    pub block: Block,
}

impl JsMethod {
    pub fn parse_impl_fn(func: ImplItemFn, self_ty: &Type) -> Self {
        let span = func.span();
        let ImplItemFn {
            mut attrs,
            vis,
            defaultness,
            sig,
            block,
        } = func;
        let parse_attrs = match ImplFnAttr::from_attributes(&attrs) {
            Ok(x) => x,
            Err(e) => {
                abort!(span, "{}", e);
            }
        };

        let attr_span = attrs
            .is_empty()
            .then_some(span)
            .unwrap_or_else(|| attrs[0].span());

        parse_attrs.validate(attr_span);

        if let Some(d) = defaultness {
            abort!(d, "specialized fn's are not supported.")
        }

        attrs.retain(|x| !x.path().is_ident("qjs"));

        let function = JsFunction::new(vis.clone(), &sig, Some(self_ty));

        JsMethod {
            attr_span,
            parse_attrs,
            function,
            attrs,
            vis,
            sig,
            block,
        }
    }

    /// The name on of this method on the javascript side.
    pub fn name(&self, case: Option<Case>) -> String {
        if let Some(x) = self.parse_attrs.rename.clone() {
            x
        } else {
            let res = self.function.name.to_string();
            if let Some(case) = case {
                res.to_case(case.to_convert_case())
            } else {
                res
            }
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
        if self.parse_attrs.skip {
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
        if self.parse_attrs.skip {
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
        if self.parse_attrs.skip {
            return TokenStream::new();
        }
        let func_name_str = self.name(case);
        let js_func_name = self.function.expand_carry_type_name(prefix);
        quote! {
            #object_name.set(#func_name_str,<#self_ty>::#js_func_name)?;
        }
    }
}
