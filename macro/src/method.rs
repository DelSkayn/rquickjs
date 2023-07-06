use std::collections::HashMap;

use darling::{FromAttributes, FromMeta};
use proc_macro2::{Ident, TokenStream};
use proc_macro_error::{abort, emit_warning};
use quote::quote;
use syn::{spanned::Spanned, Attribute, Block, ImplItemFn, ItemImpl, Signature, Type, Visibility};

use crate::{crate_ident, function::JsFunction, Common};

#[derive(Debug, FromMeta, Default)]
#[darling(default)]
pub(crate) struct AttrItem {
    prefix: Option<String>,
    #[darling(rename = "crate")]
    crate_: Option<Ident>,
}

#[derive(Debug, FromAttributes, Default, Clone)]
#[darling(default)]
#[darling(attributes(qjs))]
pub(crate) struct MethodAttr {
    new: bool,
    skip: bool,
    r#static: bool,
    configurable: bool,
    enumerable: bool,
    get: bool,
    set: bool,
    rename: Option<String>,
}

#[derive(Debug, Clone)]
pub struct JsMethod {
    parse_attrs: MethodAttr,
    function: JsFunction,
    attrs: Vec<Attribute>,
    vis: Visibility,
    sig: Signature,
    block: Block,
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
        let parse_attrs = match MethodAttr::from_attributes(&attrs) {
            Ok(x) => x,
            Err(e) => {
                abort!(span, "{}", e);
            }
        };

        if parse_attrs.get && parse_attrs.set {
            abort!(
                attrs[0],
                "a function can't both be a setter and a getter at the same time."
            )
        }

        if parse_attrs.new && parse_attrs.get {
            abort!(
                attrs[0],
                "a function can't both be a getter and a constructor at the same time."
            )
        }

        if parse_attrs.new && parse_attrs.set {
            abort!(
                attrs[0],
                "a function can't both be a setter and a constructor at the same time."
            )
        }

        if parse_attrs.configurable && !(parse_attrs.get || parse_attrs.set) {
            abort!(
                attrs[0],
                "configurable can only be set for getters and setters."
            )
        }

        if parse_attrs.enumerable && !(parse_attrs.get || parse_attrs.set) {
            abort!(
                attrs[0],
                "enumerable can only be set for getters and setters."
            )
        }

        if let Some(d) = defaultness {
            abort!(d, "specialized fn's are not supported.")
        }

        attrs.retain(|x| !x.path().is_ident("qjs"));

        let function = JsFunction::new(vis.clone(), &sig, Some(self_ty));

        JsMethod {
            parse_attrs,
            function,
            attrs,
            vis,
            sig,
            block,
        }
    }

    /// The name on of this method on the javascript side.
    pub fn name(&self) -> String {
        if let Some(x) = self.parse_attrs.rename.clone() {
            x
        } else {
            format!("{}", self.function.name)
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

    pub(crate) fn expand_js_impl(&self, common: &Common) -> TokenStream {
        if self.parse_attrs.skip {
            return TokenStream::new();
        }
        let carry_type = self.function.expand_carry_type(common);
        let impl_ = self.function.expand_to_js_function_impl(common);
        let into_js = self.function.expand_into_js_impl(common);

        quote! {
            #carry_type

            #impl_

            #into_js
        }
    }

    pub(crate) fn expand_associated_type(
        &self,
        associated_common: &Common,
        common: &Common,
    ) -> TokenStream {
        if self.parse_attrs.skip {
            return TokenStream::new();
        }
        let associated_name = self.function.expand_carry_type_name(associated_common);
        let impl_name = self.function.expand_carry_type_name(common);
        let vis = &self.vis;

        quote! {
            #vis const #associated_name: #impl_name = #impl_name;
        }
    }

    pub(crate) fn expand_apply_to_proto(&self, common: &Common, self_ty: &Type) -> TokenStream {
        if self.parse_attrs.skip {
            return TokenStream::new();
        }
        let func_name_str = &self.function.name;
        let js_func_name = self.function.expand_carry_type_name(common);
        quote! {
            _proto.set(stringify!(#func_name_str),#self_ty::#js_func_name)?;
        }
    }
}

pub struct Accessor {
    get: Option<JsMethod>,
    set: Option<JsMethod>,
}

impl Accessor {
    fn expand_impl(&self) -> TokenStream {
        let mut res = TokenStream::new();
        if let Some(ref x) = self.get {
            res.extend(x.expand_impl());
        }
        if let Some(ref x) = self.set {
            res.extend(x.expand_impl());
        }
        res
    }

    fn expand_js_impl(&self, common: &Common) -> TokenStream {
        let get_common = Common {
            prefix: "__impl_get_".to_string(),
            lib_crate: common.lib_crate.clone(),
        };
        let set_common = Common {
            prefix: "__impl_set_".to_string(),
            lib_crate: common.lib_crate.clone(),
        };

        let mut res = TokenStream::new();
        if let Some(ref g) = self.get {
            res.extend(g.expand_js_impl(&get_common));
        }
        if let Some(ref s) = self.set {
            res.extend(s.expand_js_impl(&set_common));
        }
        res
    }

    fn expand_apply_to_proto(&self, lib_crate: &Ident) -> TokenStream {
        let get_common = Common {
            prefix: "__impl_get_".to_string(),
            lib_crate: lib_crate.clone(),
        };
        let set_common = Common {
            prefix: "__impl_set_".to_string(),
            lib_crate: lib_crate.clone(),
        };

        match (self.get.as_ref(), self.set.as_ref()) {
            (Some(get), Some(set)) => {
                let configurable = get.parse_attrs.configurable || set.parse_attrs.configurable;
                let enumerable = get.parse_attrs.enumerable || set.parse_attrs.enumerable;

                let name = get.name();

                let configurable = configurable
                    .then(|| quote!(.configurable()))
                    .unwrap_or_default();
                let enumerable = enumerable
                    .then(|| quote!(.enumerable()))
                    .unwrap_or_default();

                let get_name = get.function.expand_carry_type_name(&get_common);
                let set_name = set.function.expand_carry_type_name(&set_common);
                quote! {_proto.prop(#name,
                        #lib_crate::object::Accessor::new(#get_name,#set_name)
                        #configurable
                        #enumerable
                )?;}
            }
            (Some(get), None) => {
                let configurable = get.parse_attrs.configurable;
                let enumerable = get.parse_attrs.enumerable;

                let name = get.name();

                let configurable = configurable
                    .then(|| quote!(.configurable()))
                    .unwrap_or_default();
                let enumerable = enumerable
                    .then(|| quote!(.enumerable()))
                    .unwrap_or_default();

                let get_name = get.function.expand_carry_type_name(&get_common);
                quote! {_proto.prop(#name,
                        #lib_crate::object::Accessor::new_get(#get_name)
                        #configurable
                        #enumerable
                )?;}
            }
            (None, Some(set)) => {
                let configurable = set.parse_attrs.configurable;
                let enumerable = set.parse_attrs.enumerable;

                let name = set.name();

                let configurable = configurable
                    .then(|| quote!(.configurable()))
                    .unwrap_or_default();
                let enumerable = enumerable
                    .then(|| quote!(.enumerable()))
                    .unwrap_or_default();

                let set_name = set.function.expand_carry_type_name(&set_common);
                quote! {_proto.prop(#name,
                        #lib_crate::object::Accessor::new_set(#set_name)
                        #configurable
                        #enumerable
                )?;}
            }
            (None, None) => TokenStream::new(),
        }
    }
}

pub(crate) fn expand(attr: AttrItem, item: ItemImpl) -> TokenStream {
    let ItemImpl {
        attrs,
        defaultness,
        unsafety,
        impl_token,
        generics,
        trait_,
        self_ty,
        items,
        ..
    } = item;

    if let Some(trait_) = trait_.as_ref() {
        abort!(
            trait_.2,
            "#[method] can't be applied to a trait implementation"
        );
    }

    if let Some(d) = defaultness {
        abort!(d, "specialized impl's are not supported.")
    }
    if let Some(u) = unsafety {
        abort!(u, "unsafe impl's are not supported.")
    }

    let common = Common {
        prefix: attr.prefix.unwrap_or_else(|| "js_".to_string()),
        lib_crate: attr.crate_.unwrap_or_else(crate_ident),
    };

    let mut accessors = HashMap::new();
    let mut functions = Vec::new();
    //let mut consts = Vec::new();

    for item in items {
        match item {
            syn::ImplItem::Const(_item) => {}
            syn::ImplItem::Fn(item) => {
                let function = JsMethod::parse_impl_fn(item, &self_ty);
                if function.parse_attrs.get {
                    let access = accessors.entry(function.name()).or_insert(Accessor {
                        get: None,
                        set: None,
                    });
                    if access.get.is_some() {
                        emit_warning!(function.sig, "redefined a getter for `{}`", function.name());
                    }
                    access.get = Some(function);
                } else if function.parse_attrs.set {
                    let access = accessors.entry(function.name()).or_insert(Accessor {
                        get: None,
                        set: None,
                    });
                    if access.set.is_some() {
                        emit_warning!(function.sig, "redefined a setter for `{}`", function.name());
                    }
                    access.set = Some(function.clone());
                } else {
                    functions.push(function)
                }
            }
            _ => {}
        }
    }

    let func_common = Common {
        prefix: "__impl_".to_string(),
        lib_crate: common.lib_crate.clone(),
    };

    let function_impls = functions.iter().map(|func| func.expand_impl());
    let accessor_impls = accessors.values().map(|access| access.expand_impl());

    let function_js_impls = functions
        .iter()
        .map(|func| func.expand_js_impl(&func_common));
    let accessor_js_impls = accessors
        .values()
        .map(|access| access.expand_js_impl(&common));

    let lib_crate = &common.lib_crate;

    let associated_types = functions
        .iter()
        .map(|func| func.expand_associated_type(&common, &func_common));

    let function_apply_proto = functions
        .iter()
        .map(|func| func.expand_apply_to_proto(&common, &self_ty));
    let accessor_apply_proto = accessors
        .values()
        .map(|access| access.expand_apply_to_proto(&common.lib_crate));

    quote! {
        #(#attrs)*
        #impl_token #generics #self_ty {
            #(#function_impls)*
            #(#accessor_impls)*
        }

        #(#function_js_impls)*
        #(#accessor_js_impls)*

        #[allow(non_upper_case_globals)]
        impl #self_ty{
            #(#associated_types)*
        }

        impl #lib_crate::class::impl_::MethodImplementor<#self_ty> for #lib_crate::class::impl_::MethodImpl<#self_ty> {
            fn implement<'js>(&self, _proto: &#lib_crate::Object<'js>) -> #lib_crate::Result<()>{
                #(#function_apply_proto)*
                #(#accessor_apply_proto)*
                Ok(())
            }
        }
    }
}
