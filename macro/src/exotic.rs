use proc_macro2::{Ident, Span, TokenStream};
use quote::{format_ident, quote};
use syn::{
    parse::{Parse, ParseStream},
    spanned::Spanned,
    Attribute, Block, Error, ImplItemFn, ItemImpl, Result, ReturnType, Signature, Type, Visibility,
};

use crate::{
    attrs::{take_attributes, FlagOption, OptionList},
    common::{crate_ident, kw},
    function::JsFunction,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExoticMethodKind {
    Get,
    Set,
    Delete,
    Has,
}

impl ExoticMethodKind {
    fn trait_method_name(&self) -> &'static str {
        match self {
            Self::Get => "exotic_get_property",
            Self::Set => "exotic_set_property",
            Self::Delete => "exotic_delete_property",
            Self::Has => "exotic_has_property",
        }
    }
}

#[derive(Default)]
struct ExoticMethodConfig {
    kind: Option<ExoticMethodKind>,
}

enum ExoticMethodOption {
    Get(FlagOption<kw::get>),
    Set(FlagOption<kw::set>),
    Delete(FlagOption<kw::delete>),
    Has(FlagOption<kw::has>),
}

impl Parse for ExoticMethodOption {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.peek(kw::get) {
            input.parse().map(Self::Get)
        } else if input.peek(kw::set) {
            input.parse().map(Self::Set)
        } else if input.peek(kw::delete) {
            input.parse().map(Self::Delete)
        } else if input.peek(kw::has) {
            input.parse().map(Self::Has)
        } else {
            Err(syn::Error::new(
                input.span(),
                "invalid exotic method attribute, expected one of: get, set, delete, has",
            ))
        }
    }
}

impl ExoticMethodConfig {
    fn apply(&mut self, option: &ExoticMethodOption, span: Span) -> Result<()> {
        let (new_kind, option_name) = match option {
            ExoticMethodOption::Get(x) if x.is_true() => (ExoticMethodKind::Get, "get"),
            ExoticMethodOption::Set(x) if x.is_true() => (ExoticMethodKind::Set, "set"),
            ExoticMethodOption::Delete(x) if x.is_true() => (ExoticMethodKind::Delete, "delete"),
            ExoticMethodOption::Has(x) if x.is_true() => (ExoticMethodKind::Has, "has"),
            _ => return Ok(()), // Flag is false, ignore
        };

        if let Some(existing_kind) = self.kind {
            let error = Error::new(
                span,
                format!(
                    "exotic method cannot have multiple attributes (found '{}' but already have '{}')",
                    option_name,
                    match existing_kind {
                        ExoticMethodKind::Get => "get",
                        ExoticMethodKind::Set => "set",
                        ExoticMethodKind::Delete => "delete",
                        ExoticMethodKind::Has => "has",
                    }
                ),
            );
            return Err(error);
        }

        self.kind = Some(new_kind);
        Ok(())
    }
}

struct ExoticMethod {
    kind: ExoticMethodKind,
    function: JsFunction,
    attrs: Vec<Attribute>,
    vis: Visibility,
    sig: Signature,
    block: Block,
    has_ctx: bool,
    returns_result: bool,
}

impl ExoticMethod {
    fn parse(func: ImplItemFn, self_ty: &Type) -> Result<Option<Self>> {
        let ImplItemFn {
            mut attrs,
            vis,
            sig,
            block,
            ..
        } = func;

        let mut config = ExoticMethodConfig::default();

        take_attributes(&mut attrs, |attr| {
            if !attr.path().is_ident("qjs") {
                return Ok(false);
            }

            let attr_span = attr.span();
            let options = attr.parse_args::<OptionList<ExoticMethodOption>>()?;
            for option in options.0.iter() {
                config.apply(option, attr_span)?;
            }
            Ok(true)
        })?;

        // If no exotic attributes, this is not an exotic method
        let Some(kind) = config.kind else {
            return Ok(None);
        };

        let function = JsFunction::new(vis.clone(), &sig, Some(self_ty))?;

        if function.params.params.is_empty() || !function.params.params[0].is_this {
            return Err(Error::new(
                sig.span(),
                "Exotic methods must have a self receiver",
            ));
        }

        let has_ctx = if function.params.params.len() > 1 {
            // Check if the second parameter is a Ctx
            // We need to look at the original signature for this
            sig.inputs.iter().nth(1).is_some_and(|arg| {
                if let syn::FnArg::Typed(pat_type) = arg {
                    if let Type::Path(type_path) = &*pat_type.ty {
                        type_path
                            .path
                            .segments
                            .last()
                            .is_some_and(|seg| seg.ident == "Ctx")
                    } else {
                        false
                    }
                } else {
                    false
                }
            })
        } else {
            false
        };

        // Check return type
        let returns_result = match &sig.output {
            ReturnType::Default => false,
            ReturnType::Type(_, ty) => {
                if let Type::Path(type_path) = &**ty {
                    type_path
                        .path
                        .segments
                        .first()
                        .is_some_and(|seg| seg.ident == "Result")
                } else {
                    false
                }
            }
        };

        Ok(Some(ExoticMethod {
            kind,
            function,
            attrs,
            vis,
            sig,
            block,
            has_ctx,
            returns_result,
        }))
    }

    fn expand_wrapper(&self, crate_name: &Ident, self_ty: &Type) -> TokenStream {
        let method_name = &self.function.name;
        let trait_method = format_ident!("{}", self.kind.trait_method_name());

        // Use JsFunction's params to determine if we need borrow or borrow_mut
        let borrow_call = if let Some(first_param) = self.function.params.params.first() {
            match first_param.kind {
                crate::function::ParamKind::BorrowMut => quote! { this.borrow_mut() },
                _ => quote! { this.borrow() },
            }
        } else {
            quote! { this.borrow() }
        };

        let (params, args, return_type, result_conversion) = match self.kind {
            ExoticMethodKind::Get => {
                let params = quote! { ctx: &#crate_name::Ctx<'js>, atom: #crate_name::Atom<'js>, _receiver: #crate_name::Value<'js> };
                let args = if self.has_ctx {
                    quote! { ctx, atom }
                } else {
                    quote! { atom }
                };
                let conversion = if self.returns_result {
                    quote! { result.and_then(|r| #crate_name::IntoJs::into_js(r, ctx)) }
                } else {
                    quote! { #crate_name::IntoJs::into_js(result, ctx) }
                };
                (params, args, quote! { #crate_name::Value<'js> }, conversion)
            }
            ExoticMethodKind::Set => {
                let params = quote! { ctx: &#crate_name::Ctx<'js>, atom: #crate_name::Atom<'js>, _receiver: #crate_name::Value<'js>, value: #crate_name::Value<'js> };
                let args = if self.has_ctx {
                    quote! { ctx, atom, value }
                } else {
                    quote! { atom, value }
                };
                let conversion = if self.returns_result {
                    quote! { result }
                } else {
                    quote! { Ok(result) }
                };
                (params, args, quote! { bool }, conversion)
            }
            ExoticMethodKind::Delete | ExoticMethodKind::Has => {
                let params = quote! { ctx: &#crate_name::Ctx<'js>, atom: #crate_name::Atom<'js> };
                let args = if self.has_ctx {
                    quote! { ctx, atom }
                } else {
                    quote! { atom }
                };
                let conversion = if self.returns_result {
                    quote! { result }
                } else {
                    quote! { Ok(result) }
                };
                (params, args, quote! { bool }, conversion)
            }
        };

        quote! {
            pub fn #trait_method<'js>(
                this: &#crate_name::class::JsCell<'js, #self_ty>,
                #params
            ) -> #crate_name::Result<#return_type> {
                let result = #borrow_call.#method_name(#args);
                #result_conversion
            }
        }
    }

    fn expand_impl(&self) -> TokenStream {
        let attrs = &self.attrs;
        let vis = &self.vis;
        let sig = &self.sig;
        let block = &self.block;

        quote! {
            #(#attrs)* #vis #sig #block
        }
    }
}

fn get_class_name(ty: &Type) -> String {
    match ty {
        Type::Path(x) => x.path.segments.first().unwrap().ident.to_string(),
        Type::Paren(x) => get_class_name(&x.elem),
        _ => "Unknown".to_string(),
    }
}

pub(crate) fn expand(item: ItemImpl) -> Result<TokenStream> {
    let ItemImpl {
        attrs,
        generics,
        self_ty,
        items,
        ..
    } = item;

    let crate_name = format_ident!("{}", crate_ident()?);
    let class_name = get_class_name(&self_ty);
    let module_name = format_ident!("__impl_exotic_{}__", class_name);

    let mut methods = Vec::new();
    let mut user_impls = Vec::new();

    for item in items {
        if let syn::ImplItem::Fn(func) = item {
            if let Some(method) = ExoticMethod::parse(func, &self_ty)? {
                user_impls.push(method.expand_impl());
                methods.push(method);
            }
        }
    }

    // Generate wrappers for user-provided methods
    let user_wrappers: Vec<_> = methods
        .iter()
        .map(|m| m.expand_wrapper(&crate_name, &self_ty))
        .collect();

    // Generate default implementations for missing methods
    let has_get = methods.iter().any(|m| m.kind == ExoticMethodKind::Get);
    let has_set = methods.iter().any(|m| m.kind == ExoticMethodKind::Set);
    let has_delete = methods.iter().any(|m| m.kind == ExoticMethodKind::Delete);
    let has_has = methods.iter().any(|m| m.kind == ExoticMethodKind::Has);

    let default_get = if !has_get {
        quote! {
            pub fn exotic_get_property<'js>(
                this: &#crate_name::class::JsCell<'js, #self_ty>,
                ctx: &#crate_name::Ctx<'js>,
                _atom: #crate_name::Atom<'js>,
                _receiver: #crate_name::Value<'js>,
            ) -> #crate_name::Result<#crate_name::Value<'js>> {
                let _ = this;
                Ok(#crate_name::Value::new_undefined(ctx.clone()))
            }
        }
    } else {
        TokenStream::new()
    };

    let default_set = if !has_set {
        quote! {
            pub fn exotic_set_property<'js>(
                this: &#crate_name::class::JsCell<'js, #self_ty>,
                _ctx: &#crate_name::Ctx<'js>,
                _atom: #crate_name::Atom<'js>,
                _receiver: #crate_name::Value<'js>,
                _value: #crate_name::Value<'js>,
            ) -> #crate_name::Result<bool> {
                let _ = this;
                Ok(false)
            }
        }
    } else {
        TokenStream::new()
    };

    let default_delete = if !has_delete {
        quote! {
            pub fn exotic_delete_property<'js>(
                this: &#crate_name::class::JsCell<'js, #self_ty>,
                _ctx: &#crate_name::Ctx<'js>,
                _atom: #crate_name::Atom<'js>,
            ) -> #crate_name::Result<bool> {
                let _ = this;
                Ok(false)
            }
        }
    } else {
        TokenStream::new()
    };

    let default_has = if !has_has {
        quote! {
            pub fn exotic_has_property<'js>(
                this: &#crate_name::class::JsCell<'js, #self_ty>,
                _ctx: &#crate_name::Ctx<'js>,
                _atom: #crate_name::Atom<'js>,
            ) -> #crate_name::Result<bool> {
                let _ = this;
                Ok(false)
            }
        }
    } else {
        TokenStream::new()
    };

    let res = quote! {
        #(#attrs)*
        impl #generics #self_ty {
            #(#user_impls)*
        }

        #[allow(non_snake_case)]
        mod #module_name {
            pub use super::*;

            pub(crate) struct ExoticImpl;

            impl ExoticImpl {
                #(#user_wrappers)*
                #default_get
                #default_set
                #default_delete
                #default_has
            }
        }
    };

    Ok(res)
}
