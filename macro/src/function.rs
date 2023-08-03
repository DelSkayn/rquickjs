use proc_macro2::{Ident, TokenStream};
use proc_macro_error::abort;
use quote::{format_ident, quote};
use syn::{
    fold::Fold,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    token::Comma,
    FnArg, LitStr, Signature, Token, Type, Visibility,
};

use crate::{
    attrs::{take_attributes, OptionList, ValueOption},
    common::{crate_ident, kw, AbortResultExt, SelfReplacer, BASE_PREFIX},
};

#[derive(Debug, Default)]
pub(crate) struct FunctionConfig {
    pub crate_: Option<String>,
    pub prefix: Option<String>,
    pub rename: Option<String>,
}

pub(crate) enum FunctionOption {
    Prefix(ValueOption<kw::prefix, LitStr>),
    Crate(ValueOption<Token![crate], LitStr>),
    Rename(ValueOption<kw::rename, LitStr>),
}

impl Parse for FunctionOption {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.peek(Token![crate]) {
            input.parse().map(Self::Crate)
        } else if input.peek(kw::prefix) {
            input.parse().map(Self::Prefix)
        } else if input.peek(kw::rename) {
            input.parse().map(Self::Rename)
        } else {
            Err(syn::Error::new(input.span(), "invalid class attribute"))
        }
    }
}

impl FunctionConfig {
    pub fn apply(&mut self, option: &FunctionOption) {
        match option {
            FunctionOption::Crate(ref x) => {
                self.crate_ = Some(x.value.value());
            }
            FunctionOption::Rename(ref x) => {
                self.rename = Some(x.value.value());
            }
            FunctionOption::Prefix(ref x) => {
                self.rename = Some(x.value.value());
            }
        }
    }

    pub fn crate_name(&self) -> String {
        self.crate_.clone().unwrap_or_else(crate_ident)
    }
}

pub(crate) fn expand(options: OptionList<FunctionOption>, mut item: syn::ItemFn) -> TokenStream {
    let mut config = FunctionConfig::default();
    for option in options.0.iter() {
        config.apply(option)
    }

    take_attributes(&mut item.attrs, |attr| {
        if !attr.path().is_ident("qjs") {
            return Ok(false);
        }

        let options: OptionList<FunctionOption> = attr.parse_args()?;
        for option in options.0.iter() {
            config.apply(option)
        }

        Ok(true)
    })
    .unwrap_or_abort();

    let crate_name = format_ident!("{}", config.crate_name());
    let prefix = config.prefix.unwrap_or_else(|| BASE_PREFIX.to_string());

    let func = JsFunction::new(item.vis.clone(), &item.sig, None);

    let carry_type = func.expand_carry_type(&prefix);
    let impl_ = func.expand_to_js_function_impl(&prefix, &crate_name);
    let into_js = func.expand_into_js_impl(&prefix, &crate_name);

    quote! {
        #item

        #carry_type

        #impl_

        #into_js
    }
}

#[derive(Clone)]
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
            quote! {  <#self_type >::#ident }
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

    pub fn expand_carry_type_name(&self, prefix: &str) -> Ident {
        format_ident!("{}{}", prefix, self.name)
    }
    /// Expands the type which will carry the function implementations.
    pub fn expand_carry_type(&self, prefix: &str) -> TokenStream {
        let vis = &self.vis;
        let name = self.expand_carry_type_name(prefix);
        quote! {
            #[allow(non_camel_case_types)]
            #vis struct #name;
        }
    }

    /// Expands the type which will carry the function implementations.
    pub fn expand_into_js_impl(&self, prefix: &str, lib_crate: &Ident) -> TokenStream {
        let js_name = self.expand_carry_type_name(prefix);
        quote! {
            impl<'js> #lib_crate::IntoJs<'js> for #js_name{
                fn into_js(self, ctx: &#lib_crate::Ctx<'js>) -> #lib_crate::Result<#lib_crate::Value<'js>>{
                    #lib_crate::Function::new(ctx.clone(),#js_name)?.into_js(ctx)
                }
            }
        }
    }

    pub fn expand_to_js_function_body(&self, lib_crate: &Ident) -> TokenStream {
        let arg_extract = self.params.expand_extract(lib_crate);
        let arg_apply = self.params.expand_apply();
        let rust_function = &self.rust_function;

        if self.is_async {
            quote! {
                #arg_extract

                let fut = async move {
                    #rust_function(#arg_apply).await
                };

                #lib_crate::IntoJs::into_js(#lib_crate::promise::Promised(fut), &ctx)
            }
        } else {
            quote! {
                #arg_extract
                let res = #rust_function(#arg_apply);
                #lib_crate::IntoJs::into_js(res,&ctx)
            }
        }
    }

    pub fn expand_to_js_function_impl(&self, prefix: &str, lib_crate: &Ident) -> TokenStream {
        let body = self.expand_to_js_function_body(lib_crate);
        let arg_types = self.params.expand_type(lib_crate);
        let arg_type_requirements = arg_types.iter().map(|ty| {
            quote! {
                .combine(<#ty as #lib_crate::function::FromParam>::param_requirement())
            }
        });
        let arg_type_tuple = quote!((#(#arg_types,)*));
        let js_name = self.expand_carry_type_name(prefix);

        quote! {
            impl<'js> #lib_crate::function::IntoJsFunc<'js,#arg_type_tuple> for #js_name{

                fn param_requirements() -> #lib_crate::function::ParamRequirement {
                    #lib_crate::function::ParamRequirement::none()
                    #(#arg_type_requirements)*
                }

                fn call<'a>(&self, params: #lib_crate::function::Params<'a,'js>) -> #lib_crate::Result<#lib_crate::Value<'js>>{
                    let ctx = params.ctx().clone();
                    params.check_params(Self::param_requirements())?;
                    let mut _params = params.access();
                    #body
                }

            }
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct JsParams {
    pub params: Vec<JsParam>,
}

impl JsParams {
    pub fn expand_apply(&self) -> TokenStream {
        let iter = self.params.iter().map(|x| x.expand_apply());
        quote! { #(#iter),* }
    }

    pub fn expand_type(&self, lib_crate: &Ident) -> Vec<TokenStream> {
        self.params
            .iter()
            .map(|x| x.expand_type(lib_crate))
            .collect()
    }

    pub fn expand_extract(&self, lib_crate: &Ident) -> TokenStream {
        let res = self.params.iter().map(|x| x.expand_extract(lib_crate));
        quote!(#(#res)*)
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) enum ParamKind {
    Value,
    Borrow,
    BorrowMut,
}

#[derive(Debug, Clone)]
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

    pub fn expand_extract(&self, lib_crate: &Ident) -> TokenStream {
        let ty = self.expand_type(lib_crate);
        let binding = self.expand_binding();
        quote! {
            let #binding = <#ty as #lib_crate::function::FromParam>::from_param(&mut _params)?;
        }
    }
}

impl JsParams {
    pub fn from_input(inputs: &Punctuated<FnArg, Comma>, self_type: Option<&Type>) -> Self {
        let mut types = Vec::<JsParam>::new();

        let mut self_replacer = self_type.map(SelfReplacer::with);

        for (idx, arg) in inputs.iter().enumerate() {
            match arg {
                FnArg::Typed(pat) => {
                    let (stream, kind) = match *pat.ty {
                        Type::Reference(ref borrow) => {
                            let ty = (*borrow.elem).clone();
                            let ty = if let Some(repl) = self_replacer.as_mut() {
                                repl.fold_type(ty)
                            } else {
                                ty
                            };
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
                            let ty = self_replacer
                                .as_mut()
                                .map(|x| x.fold_type(ty.clone()))
                                .unwrap_or_else(|| ty.clone());
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
}
