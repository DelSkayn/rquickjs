use std::collections::HashMap;

use darling::{FromAttributes, FromMeta};
use proc_macro2::{Ident, Span, TokenStream};
use proc_macro_error::{abort, emit_warning};
use quote::{format_ident, quote};
use syn::ItemImpl;

use crate::common::{add_js_lifetime, crate_ident, Case, BASE_PREFIX, IMPL_PREFIX};

mod accessor;
use accessor::JsAccessor;
mod method;
use method::JsMethod;

#[derive(Debug, FromMeta, Default)]
#[darling(default)]
pub(crate) struct ImplAttr {
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
pub(crate) struct ImplFnAttr {
    constructor: bool,
    skip: bool,
    r#static: bool,
    configurable: bool,
    enumerable: bool,
    get: bool,
    set: bool,
    rename: Option<String>,
}

impl ImplFnAttr {
    /// Make sure attrs aren't applied in ways they shouldn't be.
    /// Span: The span the error should be attached to.
    pub fn validate(&self, span: Span) {
        if self.get && self.set {
            abort!(
                span,
                "a function can't both be a setter and a getter at the same time."
            )
        }

        if self.constructor && self.rename.is_some() {
            emit_warning!(span, "renaming a constructor has no effect")
        }

        if self.constructor && self.get {
            abort!(
                span,
                "a function can't both be a getter and a constructor at the same time."
            )
        }

        if self.constructor && self.set {
            abort!(
                span,
                "a function can't both be a setter and a constructor at the same time."
            )
        }

        if self.configurable && !(self.get || self.set) {
            abort!(
                span,
                "configurable can only be set for getters and setters."
            )
        }

        if self.enumerable && !(self.get || self.set) {
            abort!(span, "enumerable can only be set for getters and setters.")
        }
    }
}

pub(crate) fn expand(attr: ImplAttr, item: ItemImpl) -> TokenStream {
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

    let prefix = attr.prefix.unwrap_or_else(|| BASE_PREFIX.to_string());
    let lib_crate = attr.crate_.unwrap_or_else(crate_ident);

    let mut accessors = HashMap::new();
    let mut functions = Vec::new();
    let mut constructor: Option<JsMethod> = None;
    let mut static_span: Option<Span> = None;
    //let mut consts = Vec::new();

    for item in items {
        match item {
            syn::ImplItem::Const(_item) => {}
            syn::ImplItem::Fn(item) => {
                let function = JsMethod::parse_impl_fn(item, &self_ty);
                let span = function.attr_span;
                if function.parse_attrs.get || function.parse_attrs.set {
                    let access = accessors
                        .entry(function.name(attr.rename_accessors))
                        .or_insert_with(JsAccessor::new);
                    if function.parse_attrs.get {
                        access.define_get(function, attr.rename_accessors);
                    } else {
                        access.define_set(function, attr.rename_accessors);
                    }
                } else if function.parse_attrs.constructor {
                    if let Some(first) = constructor.replace(function) {
                        let first_span = first.attr_span;
                        abort!(
                            span,
                            "A class can only have a single constructor";
                            hint = first_span => "First constructor defined here"
                        );
                    }
                } else {
                    if static_span.is_none() && function.parse_attrs.r#static {
                        static_span = Some(function.attr_span);
                    }
                    functions.push(function)
                }
            }
            _ => {}
        }
    }

    // Warn about unused static definitions if no constructor was created.
    if constructor.is_none() {
        if let Some(span) = static_span {
            emit_warning!(
                span,
                "Static methods are unused if an class doesn't have a constructor.";
                hint = "Static methods are defined on the class constructor."
            );
        }
    }

    let function_impls = functions.iter().map(|func| func.expand_impl());
    let accessor_impls = accessors.values().map(|access| access.expand_impl());
    let constructor_impl = constructor.as_ref().map(|constr| constr.expand_impl());

    let function_js_impls = functions
        .iter()
        .map(|func| func.expand_js_impl(IMPL_PREFIX, &lib_crate));
    let accessor_js_impls = accessors
        .values()
        .map(|access| access.expand_js_impl(&lib_crate));
    let constructor_js_impl = constructor
        .as_ref()
        .map(|constr| constr.expand_js_impl(IMPL_PREFIX, &lib_crate));

    let associated_types = functions
        .iter()
        .map(|func| func.expand_associated_type(&prefix, IMPL_PREFIX));

    let proto_ident = format_ident!("_proto");
    let function_apply_proto = functions.iter().filter_map(|func| {
        (!func.parse_attrs.r#static).then(|| {
            func.expand_apply_to_object(&prefix, &self_ty, &proto_ident, attr.rename_methods)
        })
    });
    let accessor_apply_proto = accessors
        .values()
        .map(|access| access.expand_apply_to_proto(&lib_crate, attr.rename_accessors));

    let constructor_ident = format_ident!("constr");

    let constructor_create = if let Some(c) = constructor.as_ref() {
        let name = c.function.expand_carry_type_name(IMPL_PREFIX);

        let js_added_generics = add_js_lifetime(&generics);

        let static_function_apply = functions.iter().filter_map(|func| {
            func.parse_attrs.r#static.then(|| {
                func.expand_apply_to_object(
                    &prefix,
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
