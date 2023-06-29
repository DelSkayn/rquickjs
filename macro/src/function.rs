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
}

impl Common {
    fn new(attr: AttrItem, sig: &Signature) -> Self {
        let lib_crate = attr.crate_.unwrap_or_else(crate_ident);
        let prefix = attr.prefix.as_deref().unwrap_or("js_");
        let js_type_name = format_ident!("{}{}", prefix, sig.ident.to_string());
        Common {
            lib_crate,
            js_type_name,
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
        ref variadic,
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
    let impls = if asyncness.is_some() {
        expand_async(sig, &common)
    } else {
        expand_sync(sig, &common)
    };

    quote! {
        #item

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

fn expand_sync(sig: &Signature, common: &Common) -> TokenStream {
    let Signature {
        ref ident,
        ref inputs,
        ..
    } = sig;
    let Common {
        ref lib_crate,
        ref js_type_name,
    } = common;

    let arg_data = to_args_data(inputs, lib_crate);
    let arg_types = arg_data.iter().map(|x| &x.stream);
    let arg_types = quote! { (#(#arg_types,)*) };
    let arg_bindings = (0..sig.inputs.len()).map(|x| format_ident!("tmp_{}", x));
    let arg_apply = arg_data.iter().enumerate().map(|(idx, x)| {
        let ident = format_ident!("tmp_{}", idx);
        if x.should_deref {
            if x.mutable {
                quote! {
                    &mut *#ident
                }
            } else {
                quote! {
                    &*#ident
                }
            }
        } else {
            quote! {
                #ident
            }
        }
    });

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
                    let res = #ident(#(#arg_apply),*);
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
    } = common;

    let arg_data = to_args_data(inputs, &lib_crate);
    let arg_types = arg_data.iter().map(|x| &x.stream);
    let arg_types = quote! { (#(#arg_types,)*) };
    let arg_bindings = (0..sig.inputs.len()).map(|x| format_ident!("tmp_{}", x));
    let arg_apply = arg_data.iter().enumerate().map(|(idx, x)| {
        let ident = format_ident!("tmp_{}", idx);
        if x.should_deref {
            if x.mutable {
                quote! {
                    &mut *#ident
                }
            } else {
                quote! {
                    &*#ident
                }
            }
        } else {
            quote! {
                #ident
            }
        }
    });

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
                        #ident(#(#arg_apply),*).await
                    };

                    #lib_crate::IntoJs::into_js(#lib_crate::promise::Promised(fut), ctx)
                }

                Box::new(__inner)
            }
        }
    }
}

pub struct ArgData {
    stream: TokenStream,
    should_deref: bool,
    mutable: bool,
}

fn to_args_data(inputs: &Punctuated<FnArg, Comma>, lib_crate: &Ident) -> Vec<ArgData> {
    let mut types = Vec::<ArgData>::new();

    for arg in inputs.iter() {
        match arg {
            FnArg::Typed(x) => types.push(to_arg_data(x, lib_crate)),
            FnArg::Receiver(x) => {
                abort!(x.self_token, "self arguments not supported in this context");
            }
        }
    }
    types
}

fn to_arg_data(pat: &PatType, lib_crate: &Ident) -> ArgData {
    match *pat.ty {
        Type::Reference(ref borrow) => {
            let ty = &borrow.elem;
            if borrow.mutability.is_some() {
                let stream = quote! {
                    #lib_crate::class::OwnedBorrowMut<'js,#ty>
                };
                ArgData {
                    stream,
                    should_deref: true,
                    mutable: true,
                }
            } else {
                let stream = quote! {
                    #lib_crate::class::OwnedBorrow<'js,#ty>
                };
                ArgData {
                    stream,
                    should_deref: true,
                    mutable: false,
                }
            }
        }
        ref x => ArgData {
            stream: quote! { #x },
            should_deref: false,
            mutable: false,
        },
    }
}
