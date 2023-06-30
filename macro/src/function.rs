use darling::FromMeta;
use proc_macro2::{Ident, TokenStream};
use proc_macro_error::{abort, emit_warning};
use quote::{format_ident, quote};
use syn::{
    punctuated::Punctuated, token::Comma, FnArg, ItemFn, Signature, Type, TypePath, Visibility,
};

use crate::{crate_ident, Common};

#[derive(Debug, FromMeta, Default)]
#[darling(default)]
pub(crate) struct AttrItem {
    #[darling(rename = "crate")]
    crate_: Option<Ident>,
    /// The ident prefix, defaults to 'js_'.
    prefix: Option<String>,
    rename: Option<String>,
}

pub(crate) fn expand(attr: AttrItem, item: ItemFn) -> TokenStream {
    let ItemFn {
        ref vis, ref sig, ..
    } = item;

    let common = Common {
        prefix: attr.prefix.unwrap_or_else(|| "js_".to_string()),
        lib_crate: attr.crate_.unwrap_or_else(crate_ident),
    };

    let func = JsFunction::new(vis.clone(), sig, None);

    let carry_type = func.expand_carry_type(&common);
    let impl_ = func.expand_to_js_function_impl(&common);
    let into_js = func.expand_into_js_impl(&common);

    quote! {
        #item

        #carry_type

        #impl_

        #into_js
    }
}

pub(crate) struct JsFunction {
    pub vis: Visibility,
    pub name: Ident,
    pub rust_function: TokenStream,
    pub is_async: bool,
    pub params: JsParams,
}

impl JsFunction {
    pub fn new(vis: Visibility, sig: &Signature, self_type: Option<&Type>) -> Self {
        let Signature {
            ref asyncness,
            ref unsafety,
            ref abi,
            ref variadic,
            ref ident,
            ref inputs,
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
        let is_async = asyncness.is_some();

        let params = JsParams::from_input(inputs, self_type);

        let rust_function = if let Some(self_type) = self_type {
            quote! {  <#self_type>::#ident }
        } else {
            quote!( #ident )
        };

        JsFunction {
            vis,
            name: ident.clone(),
            is_async,
            rust_function,
            params,
        }
    }

    pub fn expand_carry_type_name(&self, common: &Common) -> Ident {
        format_ident!("{}{}", common.prefix, self.name)
    }
    /// Expands the type which will carry the function implementations.
    pub fn expand_carry_type(&self, common: &Common) -> TokenStream {
        let vis = &self.vis;
        let name = self.expand_carry_type_name(common);
        quote! {
            #[allow(non_camel_case_types)]
            #vis struct #name;
        }
    }

    /// Expands the type which will carry the function implementations.
    pub fn expand_into_js_impl(&self, common: &Common) -> TokenStream {
        let js_name = self.expand_carry_type_name(common);
        let lib_crate = &common.lib_crate;
        quote! {
            impl<'js> #lib_crate::IntoJs<'js> for #js_name{
                fn into_js(self, ctx: #lib_crate::Ctx<'js>) -> #lib_crate::Result<#lib_crate::Value<'js>>{
                    #lib_crate::Function::new(ctx,#js_name)?.into_js(ctx)
                }
            }
        }
    }

    pub fn expand_to_js_function_body(&self, common: &Common) -> TokenStream {
        let lib_crate = &common.lib_crate;
        let arg_types = self.params.expand_type(lib_crate);
        let arg_bindings = self.params.expand_binding();
        let arg_apply = self.params.expand_apply();
        let rust_function = &self.rust_function;

        if self.is_async {
            quote! {
                let params = <#arg_types as #lib_crate::function::FromParams>::from_params(&mut params.access())?;

                let fut = async move {
                    let (#arg_bindings) = params;
                    #rust_function(#arg_apply).await
                };

                #lib_crate::IntoJs::into_js(#lib_crate::promise::Promised(fut), ctx)
            }
        } else {
            quote! {
                let (#arg_bindings) = <#arg_types as #lib_crate::function::FromParams>::from_params(&mut params.access())?;
                let res = #rust_function(#arg_apply);
                #lib_crate::IntoJs::into_js(res,ctx)
            }
        }
    }

    pub fn expand_to_js_function_impl(&self, common: &Common) -> TokenStream {
        let body = self.expand_to_js_function_body(common);
        let lib_crate = &common.lib_crate;
        let arg_types = self.params.expand_type(lib_crate);
        let js_name = self.expand_carry_type_name(common);

        quote! {
            impl<'js> #lib_crate::function::ToJsFunction<'js,#arg_types> for #js_name{

                fn param_requirements() -> #lib_crate::function::ParamReq {
                    <#arg_types as #lib_crate::function::FromParams>::params_required()
                }

                fn to_js_function(self) -> Box<dyn #lib_crate::function::JsFunction<'js> + 'js>{
                    fn __inner<'js>(params: #lib_crate::function::Params<'_,'js>) -> #lib_crate::Result<rquickjs::Value<'js>>{
                        let ctx = params.ctx();
                        params.check_params(<#arg_types as #lib_crate::function::FromParams>::params_required())?;
                        #body
                    }

                    Box::new(__inner)
                }

            }
        }
    }
}

pub(crate) struct JsParams {
    pub params: Vec<JsParam>,
}

impl JsParams {
    pub fn expand_binding(&self) -> TokenStream {
        let iter = self.params.iter().map(|x| x.expand_binding());
        quote! { #(#iter),* }
    }

    pub fn expand_apply(&self) -> TokenStream {
        let iter = self.params.iter().map(|x| x.expand_apply());
        quote! { #(#iter),* }
    }

    pub fn expand_type(&self, lib_crate: &Ident) -> TokenStream {
        let iter = self.params.iter().map(|x| x.expand_type(lib_crate));
        quote! { (#(#iter,)*) }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum ParamKind {
    Value,
    Borrow,
    BorrowMut,
}

pub(crate) struct JsParam {
    kind: ParamKind,
    number: usize,
    tokens: TokenStream,
    is_this: bool,
}

impl JsParam {
    pub fn expand_binding(&self) -> TokenStream {
        let tmp = format_ident!("tmp_{}", self.number);
        if let ParamKind::BorrowMut = self.kind {
            quote! { mut #tmp }
        } else {
            quote! { #tmp }
        }
    }

    pub fn expand_apply(&self) -> TokenStream {
        let t = format_ident!("tmp_{}", self.number);
        let apply = match self.kind {
            ParamKind::Value => quote!(#t),
            ParamKind::Borrow => quote!(&*#t),
            ParamKind::BorrowMut => quote!(&mut *#t),
        };
        if self.is_this {
            quote!(#apply.0)
        } else {
            apply
        }
    }

    pub fn expand_type(&self, lib_crate: &Ident) -> TokenStream {
        let t = &self.tokens;
        let ty = match self.kind {
            ParamKind::Value => quote!(#t),
            ParamKind::Borrow => quote!(#lib_crate::class::OwnedBorrow<'js,#t>),
            ParamKind::BorrowMut => quote!(#lib_crate::class::OwnedBorrowMut<'js,#t>),
        };
        if self.is_this {
            quote!(
                #lib_crate::function::This<#ty>
            )
        } else {
            ty
        }
    }
}

impl JsParams {
    pub fn from_input(inputs: &Punctuated<FnArg, Comma>, self_type: Option<&Type>) -> Self {
        let mut types = Vec::<JsParam>::new();

        for (idx, arg) in inputs.iter().enumerate() {
            match arg {
                FnArg::Typed(pat) => {
                    let (stream, kind) = match *pat.ty {
                        Type::Reference(ref borrow) => {
                            let ty = Self::inner_type_to_type(&borrow.elem, self_type);
                            let stream = quote! {
                                #ty
                            };
                            let kind = if borrow.mutability.is_some() {
                                ParamKind::BorrowMut
                            } else {
                                ParamKind::Borrow
                            };
                            (stream, kind)
                        }
                        ref ty => {
                            let ty = Self::inner_type_to_type(ty, self_type);
                            (
                                quote! {
                                    #ty
                                },
                                ParamKind::Value,
                            )
                        }
                    };

                    types.push(JsParam {
                        kind,
                        tokens: stream,
                        number: idx,
                        is_this: false,
                    });
                }
                FnArg::Receiver(recv) => {
                    if let Some(self_type) = self_type {
                        let stream = quote! {
                            #self_type
                        };
                        let kind = if recv.reference.is_some() {
                            if recv.mutability.is_some() {
                                ParamKind::BorrowMut
                            } else {
                                ParamKind::Borrow
                            }
                        } else {
                            ParamKind::Value
                        };
                        types.push(JsParam {
                            kind,
                            number: idx,
                            tokens: stream,
                            is_this: true,
                        })
                    } else {
                        abort!(
                            recv.self_token,
                            "self arguments not supported in this context"
                        );
                    }
                }
            }
        }
        JsParams { params: types }
    }

    fn inner_type_to_type<'a>(ty: &'a Type, self_type: Option<&'a Type>) -> &'a Type {
        if let Type::Path(ref path) = ty {
            if let Some(first) = path.path.segments.first() {
                if first.ident == format_ident!("Self") {
                    if let Some(self_type) = self_type {
                        return self_type;
                    } else {
                        abort!(ty, "Self not supported as a argument type in this constext")
                    }
                }
            }
        }
        ty
    }
}
