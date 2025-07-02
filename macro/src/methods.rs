use std::collections::HashMap;

use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::{
    parse::{Parse, ParseStream},
    spanned::Spanned,
    Error, ItemImpl, LitStr, Result, Token, Type,
};

use crate::{
    attrs::{take_attributes, OptionList, ValueOption},
    common::{add_js_lifetime, crate_ident, kw, Case, BASE_PREFIX, IMPL_PREFIX},
};

mod accessor;
use accessor::JsAccessor;
mod method;
use method::Method;

#[derive(Default)]
pub(crate) struct ImplConfig {
    pub(crate) prefix: Option<String>,
    pub(crate) crate_: Option<String>,
    pub(crate) rename_all: Option<Case>,
}

impl ImplConfig {
    pub fn apply(&mut self, option: &ImplOption) {
        match option {
            ImplOption::Prefix(x) => {
                self.prefix = Some(x.value.value());
            }
            ImplOption::Crate(x) => {
                self.crate_ = Some(x.value.value());
            }
            ImplOption::RenameAll(x) => {
                self.rename_all = Some(x.value);
            }
        }
    }
}

pub(crate) enum ImplOption {
    Prefix(ValueOption<kw::prefix, LitStr>),
    Crate(ValueOption<Token![crate], LitStr>),
    RenameAll(ValueOption<kw::rename_all, Case>),
}

impl Parse for ImplOption {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.peek(kw::prefix) {
            input.parse().map(Self::Prefix)
        } else if input.peek(Token![crate]) {
            input.parse().map(Self::Crate)
        } else if input.peek(kw::rename_all) {
            input.parse().map(Self::RenameAll)
        } else {
            Err(syn::Error::new(input.span(), "invalid impl attribute"))
        }
    }
}

pub fn get_class_name(ty: &Type) -> String {
    match ty {
        Type::Array(_) => todo!(),
        Type::Paren(x) => get_class_name(&x.elem),
        Type::Path(x) => x.path.segments.first().unwrap().ident.to_string(),
        Type::Tuple(x) => {
            let name = x
                .elems
                .iter()
                .map(get_class_name)
                .collect::<Vec<_>>()
                .join("_");

            format!("tuple_{name}")
        }
        _ => todo!(),
    }
}

pub(crate) fn expand(options: OptionList<ImplOption>, item: ItemImpl) -> Result<TokenStream> {
    let mut config = ImplConfig::default();
    for option in options.0.iter() {
        config.apply(option)
    }

    let ItemImpl {
        mut attrs,
        defaultness,
        unsafety,
        impl_token,
        generics,
        trait_,
        self_ty,
        items,
        ..
    } = item;

    take_attributes(&mut attrs, |attr| {
        if !attr.path().is_ident("qjs") {
            return Ok(false);
        }

        let options: OptionList<ImplOption> = attr.parse_args()?;
        for option in options.0.iter() {
            config.apply(option)
        }

        Ok(true)
    })?;

    if let Some(trait_) = trait_.as_ref() {
        return Err(Error::new(
            trait_.2.span(),
            "#[method] can't be applied to a trait implementation",
        ));
    }

    if let Some(d) = defaultness {
        return Err(Error::new(
            d.span(),
            "specialized impl's are not supported.",
        ));
    }
    if let Some(u) = unsafety {
        return Err(Error::new(u.span(), "unsafe impl's are not supported."));
    }

    let prefix = config.prefix.unwrap_or_else(|| BASE_PREFIX.to_string());
    let crate_name = format_ident!("{}", config.crate_.map(Ok).unwrap_or_else(crate_ident)?);

    let mut accessors = HashMap::new();
    let mut functions = Vec::new();
    let mut constructor: Option<Method> = None;
    let mut static_span: Option<Span> = None;
    //let mut consts = Vec::new();

    for item in items {
        match item {
            syn::ImplItem::Const(_item) => {}
            syn::ImplItem::Fn(item) => {
                let function = Method::parse_impl_fn(item, &self_ty)?;
                let span = function.attr_span;
                if function.config.get || function.config.set {
                    let access = accessors
                        .entry(function.name(config.rename_all))
                        .or_insert_with(JsAccessor::new);
                    if function.config.get {
                        access.define_get(function, config.rename_all)?;
                    } else {
                        access.define_set(function, config.rename_all)?;
                    }
                } else if function.config.constructor {
                    if let Some(first) = constructor.replace(function) {
                        let first_span = first.attr_span;
                        let mut error =
                            Error::new(span, "A class can only have a single constructor");
                        error.extend(Error::new(first_span, "First constructor defined here"));
                        return Err(error);
                    }
                } else {
                    if static_span.is_none() && function.config.r#static {
                        static_span = Some(function.attr_span);
                    }
                    functions.push(function)
                }
            }
            _ => {}
        }
    }

    // Warn about unused static definitions if no constructor was created.
    /* if constructor.is_none() {
        if let Some(span) = static_span {
            emit_warning!(
                span,
                "Static methods are unused if an class doesn't have a constructor.";
                hint = "Static methods are defined on the class constructor."
            );
        }
    }*/

    let function_impls = functions.iter().map(|func| func.expand_impl());
    let accessor_impls = accessors.values().map(|access| access.expand_impl());
    let constructor_impl = constructor.as_ref().map(|constr| constr.expand_impl());

    let function_js_impls = functions
        .iter()
        .map(|func| func.expand_js_impl(IMPL_PREFIX, &crate_name));
    let accessor_js_impls = accessors
        .values()
        .map(|access| access.expand_js_impl(&crate_name));
    let constructor_js_impl = constructor
        .as_ref()
        .map(|constr| constr.expand_js_impl(IMPL_PREFIX, &crate_name));

    let associated_types = functions
        .iter()
        .map(|func| func.expand_associated_type(&prefix, IMPL_PREFIX));

    let proto_ident = format_ident!("_proto");
    let function_apply_proto = functions
        .iter()
        .filter(|&func| (!func.config.r#static))
        .map(|func| {
            func.expand_apply_to_object(&prefix, &self_ty, &proto_ident, config.rename_all)
        });
    let accessor_apply_proto = accessors
        .values()
        .map(|access| access.expand_apply_to_proto(&crate_name, config.rename_all));

    let constructor_ident = format_ident!("constr");

    let constructor_create = if let Some(c) = constructor.as_ref() {
        let name = c.function.expand_carry_type_name(IMPL_PREFIX);

        let js_added_generics = add_js_lifetime(&generics);

        let static_function_apply =
            functions
                .iter()
                .filter(|&func| func.config.r#static)
                .map(|func| {
                    func.expand_apply_to_object(
                        &prefix,
                        &self_ty,
                        &constructor_ident,
                        config.rename_all,
                    )
                });

        quote! {
            impl #js_added_generics #crate_name::class::impl_::ConstructorCreator<'js,#self_ty> for #crate_name::class::impl_::ConstructorCreate<#self_ty> {
                fn create_constructor(&self, ctx: &#crate_name::Ctx<'js>) -> #crate_name::Result<Option<#crate_name::function::Constructor<'js>>>{
                    let constr = #crate_name::function::Constructor::new_class::<#self_ty,_,_>(ctx.clone(),#name)?;
                    #(#static_function_apply)*
                    Ok(Some(constr))
                }
            }
        }
    } else {
        TokenStream::new()
    };

    let class_name = get_class_name(&self_ty);
    let impl_mod_name = format_ident!("__impl_methods_{class_name}__");

    let res = quote! {
        #(#attrs)*
        #impl_token #generics #self_ty {
            #(#function_impls)*
            #(#accessor_impls)*
            #constructor_impl
        }


        mod #impl_mod_name{
            pub use super::*;
            #(#function_js_impls)*
            #(#accessor_js_impls)*
            #constructor_js_impl

            #[allow(non_upper_case_globals)]
            impl #generics #self_ty{
                #(#associated_types)*
            }

            impl #generics #crate_name::class::impl_::MethodImplementor<#self_ty> for #crate_name::class::impl_::MethodImpl<#self_ty> {
                fn implement(&self, _proto: &#crate_name::Object<'_>) -> #crate_name::Result<()>{
                    #(#function_apply_proto)*
                    #(#accessor_apply_proto)*
                    Ok(())
                }
            }

            #constructor_create
        }
    };

    Ok(res)
}
