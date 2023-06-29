use darling::FromMeta;
use proc_macro2::{Ident, TokenStream};
use proc_macro_error::abort;
use quote::{format_ident, quote};
use syn::{
    punctuated::Punctuated, token::Comma, FnArg, ItemFn, LitBool, PatType, ReturnType, Signature,
    Type,
};

use crate::crate_ident;

#[derive(Debug, FromMeta, Default)]
#[darling(default)]
pub(crate) struct AttrItem {
    #[darling(rename = "crate")]
    crate_: Option<Ident>,
    /// The ident prefix, defaults to 'js_'.
    prefix: Option<String>,
    rename: Option<String>,
    #[darling(rename = "try")]
    try_: Option<bool>,
}

pub(crate) fn expand(attr: AttrItem, item: ItemFn) -> TokenStream {
    let ItemFn {
        ref vis, ref sig, ..
    } = item;

    let Signature {
        ref asyncness,
        ref unsafety,
        ref abi,
        ref ident,
        ref generics,
        ref inputs,
        ref variadic,
        ref output,
        ..
    } = sig;

    let lib_crate = attr.crate_.unwrap_or_else(crate_ident);
    if let Some(unsafe_) = unsafety {
        abort!(
            unsafe_,
            "implementing javascript callbacks for unsafe functions is not allowed."
        )
    }
    if let Some(abi) = abi {
        abort!(
            abi,
            "implementing javascript callbacks functions with an non rust abi is not supported."
        )
    }
    if let Some(variadic) = variadic {
        abort!(variadic,"implementing javascript callbacks for functions with variadic params is not supported.")
    }

    let prefix = attr.prefix.as_deref().unwrap_or("js_");

    let is_async = asyncness.is_some();
    let arg_types = to_args_types(inputs);
    let arg_bindings = (0..inputs.len())
        .map(|x| format_ident!("tmp_{}", x))
        .collect::<Vec<_>>();
    let name = format_ident!("{}{}", prefix, ident.to_string());
    let output_type = match output {
        ReturnType::Default => quote! { () },
        ReturnType::Type(_, t) => quote! { #t },
    };

    let try_stmt = if attr.try_.unwrap_or(false) {
        quote! { let res = res? }
    } else {
        TokenStream::new()
    };

    let is_async_literal = if is_async {
        quote! { true }
    } else {
        quote! { false }
    };

    let future_stmt = if is_async {
        quote! {
            let res = #lib_crate::promise::Promised(res);
        }
    } else {
        TokenStream::new()
    };

    quote! {
        #item

        #[allow(non_camel_case_types)]
        #vis struct #name;

        impl<'js> #lib_crate::function::ToJsFunction<'js,#arg_types, #is_async_literal> for #name{
            type Output = #output_type;

            fn param_requirements() -> #lib_crate::function::ParamReq {
                <#arg_types as #lib_crate::function::FromParams>::params_required()
            }

            fn to_js_function(self) -> Box<dyn #lib_crate::function::JsFunction<'js> + 'js>{
                fn __inner<'js>(params: #lib_crate::function::Params<'_,'js>) -> #lib_crate::Result<rquickjs::Value<'js>>{
                    let ctx = params.ctx();
                    params.check_params(<#arg_types as #lib_crate::function::FromParams>::params_required())?;
                    let (#(#arg_bindings),*) = <#arg_types as #lib_crate::function::FromParams>::from_params(&mut params.access())?;
                    let res = #ident(#(#arg_bindings),*);
                    #try_stmt;
                    #future_stmt;
                    #lib_crate::IntoJs::into_js(res,ctx)
                }
            }

        }

        impl<'js> #lib_crate::IntoJs<'js> for #name{
            fn into_js(self, ctx: #lib_crate::Ctx<'js>) -> #lib_crate::Result<#lib_crate::Value<'js>>{
                #lib_crate::Function::new(ctx,#name)?.into_js(ctx)
            }
        }
    }
}

fn to_args_types(inputs: &Punctuated<FnArg, Comma>) -> TokenStream {
    let mut types = Vec::<TokenStream>::new();

    for arg in inputs.iter() {
        match arg {
            FnArg::Typed(x) => types.push(to_arg_type(x)),
            FnArg::Receiver(x) => {
                abort!(x.self_token, "self arguments not supported in this context");
            }
        }
    }
    quote! {
        (#(#types,)*)
    }
}

fn to_arg_type(pat: &PatType) -> TokenStream {
    match *pat.ty {
        Type::Reference(_) => todo!(),
        ref x => quote! { #x },
    }
}
