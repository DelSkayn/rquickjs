use std::collections::HashMap;

use convert_case::Casing;
use darling::{FromAttributes, FromMeta};
use proc_macro2::{Ident, Span, TokenStream};
use proc_macro_error::{abort, emit_warning};
use quote::{format_ident, quote};
use syn::{spanned::Spanned, Attribute, Block, ImplItemFn, ItemImpl, Signature, Type, Visibility};

use crate::{
    class::add_js_lifetime,
    common::{crate_ident, Case},
    function::JsFunction,
    Common,
};

#[derive(Debug, FromMeta, Default)]
#[darling(default)]
pub(crate) struct AttrItem {
    prefix: Option<String>,
    #[darling(rename = "crate")]
    crate_: Option<Ident>,
    rename_methods: Option<Case>,
    rename_accessors: Option<Case>,
    rename_constants: Option<Case>,
}

#[derive(Debug, FromAttributes, Default, Clone)]
#[darling(default)]
#[darling(attributes(qjs))]
pub(crate) struct MethodAttr {
    constructor: bool,
    skip: bool,
    r#static: bool,
    configurable: bool,
    enumerable: bool,
    get: bool,
    set: bool,
    rename: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct JsMethod {
    pub attr_span: Span,
    pub parse_attrs: MethodAttr,
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
        let parse_attrs = match MethodAttr::from_attributes(&attrs) {
            Ok(x) => x,
            Err(e) => {
                abort!(span, "{}", e);
            }
        };
        let attr_span = attrs
            .is_empty()
            .then_some(span)
            .unwrap_or_else(|| attrs[0].span());

        if parse_attrs.get && parse_attrs.set {
            abort!(
                attr_span,
                "a function can't both be a setter and a getter at the same time."
            )
        }

        if parse_attrs.constructor && parse_attrs.rename.is_some() {
            emit_warning!(attr_span, "renaming a constructor has no effect")
        }

        if parse_attrs.constructor && parse_attrs.get {
            abort!(
                attr_span,
                "a function can't both be a getter and a constructor at the same time."
            )
        }

        if parse_attrs.constructor && parse_attrs.set {
            abort!(
                attr_span,
                "a function can't both be a setter and a constructor at the same time."
            )
        }

        if parse_attrs.configurable && !(parse_attrs.get || parse_attrs.set) {
            abort!(
                attr_span,
                "configurable can only be set for getters and setters."
            )
        }

        if parse_attrs.enumerable && !(parse_attrs.get || parse_attrs.set) {
            abort!(
                attr_span,
                "enumerable can only be set for getters and setters."
            )
        }

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

    pub(crate) fn expand_apply_to_object(
        &self,
        common: &Common,
        self_ty: &Type,
        object_name: &Ident,
        case: Option<Case>,
    ) -> TokenStream {
        if self.parse_attrs.skip {
            return TokenStream::new();
        }
        let func_name_str = self.name(case);
        let js_func_name = self.function.expand_carry_type_name(common);
        quote! {
            #object_name.set(#func_name_str,<#self_ty>::#js_func_name)?;
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

    fn expand_apply_to_proto(&self, lib_crate: &Ident, case: Option<Case>) -> TokenStream {
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

                let name = get.name(case);

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

                let name = get.name(case);

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

                let name = set.name(case);

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
    let mut constructor: Option<JsMethod> = None;
    //let mut consts = Vec::new();

    for item in items {
        match item {
            syn::ImplItem::Const(_item) => {}
            syn::ImplItem::Fn(item) => {
                let function = JsMethod::parse_impl_fn(item, &self_ty);
                if function.parse_attrs.get {
                    let access = accessors
                        .entry(function.name(attr.rename_accessors))
                        .or_insert(Accessor {
                            get: None,
                            set: None,
                        });
                    if let Some(first) = access.get.take() {
                        let first_span = first.attr_span;
                        emit_warning!(
                            function.attr_span, "Redefined a getter for `{}`.", function.name(attr.rename_accessors);
                            hint = first_span => "Getter first defined here."
                        );
                    }
                    access.get = Some(function);
                } else if function.parse_attrs.set {
                    let access = accessors.entry(function.name(None)).or_insert(Accessor {
                        get: None,
                        set: None,
                    });
                    if let Some(first) = access.set.take() {
                        let first_span = first.attr_span;
                        emit_warning!(
                            function.attr_span, "Redefined a setter for `{}`", function.name(attr.rename_accessors);
                            hint = first_span => "Setter first defined here"
                        );
                    }
                    access.set = Some(function.clone());
                } else if function.parse_attrs.constructor {
                    if let Some(first) = constructor.as_ref() {
                        let first_span = first.attr_span;
                        abort!(
                            function.attr_span,
                            "A class can only have a single constructor";
                            hint = first_span => "First constructor defined here"
                        );
                    }
                    constructor = Some(function);
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
    let constructor_impl = constructor.as_ref().map(|constr| constr.expand_impl());

    let function_js_impls = functions
        .iter()
        .map(|func| func.expand_js_impl(&func_common));
    let accessor_js_impls = accessors
        .values()
        .map(|access| access.expand_js_impl(&common));
    let constructor_js_impl = constructor
        .as_ref()
        .map(|constr| constr.expand_js_impl(&func_common));

    let lib_crate = &common.lib_crate;

    let associated_types = functions
        .iter()
        .map(|func| func.expand_associated_type(&common, &func_common));

    let proto_ident = format_ident!("_proto");
    let function_apply_proto = functions.iter().filter_map(|func| {
        (!func.parse_attrs.r#static).then(|| {
            func.expand_apply_to_object(&common, &self_ty, &proto_ident, attr.rename_methods)
        })
    });
    let accessor_apply_proto = accessors
        .values()
        .map(|access| access.expand_apply_to_proto(&common.lib_crate, attr.rename_accessors));

    let constructor_ident = format_ident!("constr");

    let constructor_create = if let Some(c) = constructor.as_ref() {
        let name = c.function.expand_carry_type_name(&func_common);

        let js_added_generics = add_js_lifetime(&generics);

        let static_function_apply = functions.iter().filter_map(|func| {
            func.parse_attrs.r#static.then(|| {
                func.expand_apply_to_object(
                    &common,
                    &self_ty,
                    &constructor_ident,
                    attr.rename_methods,
                )
            })
        });

        quote! {
            impl #js_added_generics #lib_crate::class::impl_::ConstructorCreator<'js,#self_ty> for #lib_crate::class::impl_::ConstructorCreate<#self_ty> {
                fn create_constructor(&self, ctx: &#lib_crate::Ctx<'js>) -> #lib_crate::Result<Option<#lib_crate::function::Constructor<'js>>>{
                    let constr = #lib_crate::function::Constructor::new_class::<#self_ty,_,_>(ctx.clone(),#name)?;
                    #(#static_function_apply)*
                    Ok(Some(constr))
                }
            }
        }
    } else {
        if let Some(x) = functions.iter().find(|x| x.parse_attrs.r#static) {
            abort!(x.attr_span,"Defined a static method on an class without a constructor"; 
                hint = "Static methods are defined on constructors");
        }

        TokenStream::new()
    };

    quote! {
        #(#attrs)*
        #impl_token #generics #self_ty {
            #(#function_impls)*
            #(#accessor_impls)*
            #constructor_impl
        }

        #(#function_js_impls)*
        #(#accessor_js_impls)*
        #constructor_js_impl

        #[allow(non_upper_case_globals)]
        impl #generics #self_ty{
            #(#associated_types)*
        }

        impl #generics #lib_crate::class::impl_::MethodImplementor<#self_ty> for #lib_crate::class::impl_::MethodImpl<#self_ty> {
            fn implement(&self, _proto: &#lib_crate::Object<'_>) -> #lib_crate::Result<()>{
                #(#function_apply_proto)*
                #(#accessor_apply_proto)*
                Ok(())
            }
        }

        #constructor_create
    }
}
