use darling::FromMeta;
use proc_macro2::{Ident, TokenStream};
use proc_macro_error::abort;
use quote::{format_ident, quote};
use syn::{punctuated::Punctuated, token::Comma, FnArg, ItemFn, PatType, Signature, Type};

use crate::crate_ident;

#[derive(Debug, FromMeta, Default)]
#[darling(default)]
pub(crate) struct AttrItem {
    #[darling(rename = "crate")]
    crate_: Option<Ident>,
    /// The ident prefix, defaults to 'js_'.
    prefix: Option<String>,
    rename: Option<String>,
}

pub struct Common {
    lib_crate: Ident,
    js_type_name: Ident,
    arg_bindings: Vec<Ident>,
}

impl Common {
    fn new(attr: AttrItem, sig: &Signature) -> Self {
        let lib_crate = attr.crate_.unwrap_or_else(crate_ident);
        let prefix = attr.prefix.as_deref().unwrap_or("js_");
        let js_type_name = format_ident!("{}{}", prefix, sig.ident.to_string());
        let arg_bindings = (0..sig.inputs.len())
            .map(|x| format_ident!("tmp_{}", x))
            .collect::<Vec<_>>();
        Common {
            lib_crate,
            js_type_name,
            arg_bindings,
        }
    }
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

    let common = Common::new(attr, sig);
    let name = &common.js_type_name;
    let lib_crate = &common.lib_crate;
    if asyncness.is_some() {
        let impls = expand_async(sig, &common);
        quote! {
            #item;

            #[allow(non_camel_case_types)]
            #vis struct #name;

            #impls;

            impl<'js> #lib_crate::IntoJs<'js> for #name{
                fn into_js(self, ctx: #lib_crate::Ctx<'js>) -> #lib_crate::Result<#lib_crate::Value<'js>>{
                    #lib_crate::Function::new(ctx,#name)?.into_js(ctx)
                }
            }
        }
    } else {
        let impls = expand_sync(sig, &common);
        quote! {
            #item;

            #[allow(non_camel_case_types)]
            #vis struct #name;

            #impls

            impl<'js> #lib_crate::IntoJs<'js> for #name{
                fn into_js(self, ctx: #lib_crate::Ctx<'js>) -> #lib_crate::Result<#lib_crate::Value<'js>>{
                    #lib_crate::Function::new(ctx,#name)?.into_js(ctx)
                }
            }
        }
    }
}

fn expand_sync(sig: &Signature, common: &Common) -> TokenStream {
    let Signature {
        ref ident,
        ref inputs,
        ..
    } = sig;
    let Common {
        ref lib_crate,
        ref js_type_name,
        ref arg_bindings,
    } = common;

    let arg_types = to_args_types(inputs);
    quote! {
        impl<'js> #lib_crate::function::ToJsFunction<'js,#arg_types> for #js_type_name{

            fn param_requirements() -> #lib_crate::function::ParamReq {
                <#arg_types as #lib_crate::function::FromParams>::params_required()
            }

            fn to_js_function(self) -> Box<dyn #lib_crate::function::JsFunction<'js> + 'js>{
                fn __inner<'js>(params: #lib_crate::function::Params<'_,'js>) -> #lib_crate::Result<rquickjs::Value<'js>>{
                    let ctx = params.ctx();
                    params.check_params(<#arg_types as #lib_crate::function::FromParams>::params_required())?;
                    let (#(#arg_bindings),*) = <#arg_types as #lib_crate::function::FromParams>::from_params(&mut params.access())?;
                    let res = #ident(#(#arg_bindings),*);
                    #lib_crate::IntoJs::into_js(res,ctx)
                }

                Box::new(__inner)
            }

        }
    }
}

fn expand_async(sig: &Signature, common: &Common) -> TokenStream {
    let Signature {
        ref ident,
        ref inputs,
        ..
    } = sig;
    let Common {
        ref lib_crate,
        ref js_type_name,
        ref arg_bindings,
    } = common;

    let arg_types = to_args_types(inputs);

    quote! {
        impl<'js> #lib_crate::function::ToJsFunction<'js,#arg_types> for #js_type_name{

            fn param_requirements() -> #lib_crate::function::ParamReq {
                <#arg_types as #lib_crate::function::FromParams>::params_required()
            }

            fn to_js_function(self) -> Box<dyn #lib_crate::function::JsFunction<'js> + 'js>{
                fn __inner<'js>(params: #lib_crate::function::Params<'_,'js>) -> #lib_crate::Result<rquickjs::Value<'js>>{
                    let ctx = params.ctx();
                    params.check_params(<#arg_types as #lib_crate::function::FromParams>::params_required())?;
                    let params = <#arg_types as #lib_crate::function::FromParams>::from_params(&mut params.access())?;

                    let fut = async move {
                        let (#(#arg_bindings),*) = params;
                        #ident(#(#arg_bindings),*).await
                    };

                    #lib_crate::promise::Promised(fut).into_js(ctx)
                }

                Box::new(__inner)
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
