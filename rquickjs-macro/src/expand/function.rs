use crate::{abort, Expander, Tokens};
use quote::quote;
use syn::{punctuated::Punctuated, FnArg, Ident, ItemFn, Pat, ReturnType, Signature, Token, Type};

impl Expander {
    /// Expand function
    pub fn function(
        &self,
        path: &Vec<&Ident>,
        ItemFn {
            sig:
                Signature {
                    asyncness,
                    unsafety,
                    ident,
                    inputs,
                    variadic,
                    output,
                    ..
                },
            ..
        }: &ItemFn,
    ) -> Tokens {
        if let Some(unsafety) = unsafety {
            abort!(
                unsafety.span,
                "Binding of unsafe functions is not supported."
            );
        }
        if let Some(variadic) = variadic {
            abort!(
                variadic.dots.spans[0],
                "Binding of variadic functions is not supported."
            );
        }
        let is_async = asyncness.is_some();

        let lib_crate = &self.lib_crate;
        let name = format!("{}", ident);
        let path = self.path(path, ident);

        let arg_names = self.arg_names(inputs);
        let arg_types = self.arg_types(inputs);

        let fn_call = quote! { #path(#(#arg_names),*) };

        let fn_call = if is_async {
            quote! { #fn_call.await }
        } else {
            fn_call
        };

        let ret_kind = self.return_kind(output);

        let fn_call = match ret_kind {
            None => quote! {
                #fn_call;
                Ok(())
            },
            Some(true) => quote! {
                #fn_call.map_err(|error| error.into())
            },
            Some(false) => quote! {
                Ok(#fn_call)
            },
        };

        let fn_call = if is_async {
            quote! { Ok(#lib_crate::PromiseJs(async move { #fn_call })) }
        } else {
            fn_call
        };

        let arg_names_decl = match arg_names.len() {
            0 => quote! { _ },
            //1 => quote! { #arg_names },
            _ => quote! { (#(#arg_names,)*) },
        };

        let arg_types_decl = match arg_types.len() {
            0 => quote! { () },
            //1 => quote! { #arg_types },
            _ => quote! { (#(#arg_types,)*) },
        };

        quote! {
            obj.set(#name, #lib_crate::Function::new(ctx, #name, |ctx: #lib_crate::Ctx<'_>, this: #lib_crate::Value<'_>, #arg_names_decl: #arg_types_decl| -> #lib_crate::Result<_> {
                #fn_call
            })?)
        }
    }

    /// Extract function arguments names (excluding self)
    pub fn arg_names<'a>(&self, args: &'a Punctuated<FnArg, Token![,]>) -> Vec<&'a Ident> {
        args.iter()
            .filter_map(|arg| match arg {
                FnArg::Receiver(_) => None,
                FnArg::Typed(arg) => Some(match &*arg.pat {
                    Pat::Ident(pat) => &pat.ident,
                    _ => abort!(arg.colon_token, "Only named arguments is supported."),
                }),
            })
            .collect()
    }

    /// Extract function arguments types (excluding self)
    pub fn arg_types<'a>(&self, args: &'a Punctuated<FnArg, Token![,]>) -> Vec<&'a Type> {
        args.iter()
            .filter_map(|arg| match arg {
                FnArg::Receiver(_recv) => None,
                FnArg::Typed(arg) => Some(&*arg.ty),
            })
            .collect()
    }

    /// Analyze function output
    pub fn return_kind(&self, output: &ReturnType) -> Option<bool> {
        match output {
            // Function does not returns anything
            ReturnType::Default => return None,
            ReturnType::Type(_, type_) => {
                if let Type::Path(path) = &**type_ {
                    match path.path.segments.last() {
                        // Function can return value or throw exception
                        Some(seg) if seg.ident == "Result" => return Some(true),
                        _ => (),
                    }
                }
            }
        }
        // Function returns value and cannot throw exception
        Some(false)
    }
}

#[cfg(test)]
mod test {
    use crate::*;
    use quote::quote;
    use syn::parse_quote;

    #[test]
    fn fn_no_args_no_return() {
        let item = parse_quote! {
            fn doit() {}
        };

        let expander = Expander::new();
        let path = Vec::new();

        let actual = expander.function(&path, &item);
        let expected = quote! {
            obj.set("doit", rquickjs::Function::new(ctx, "doit", |ctx: rquickjs::Ctx<'_>, this: rquickjs::Value<'_>, _: ()| -> rquickjs::Result<_> {
                doit();
                Ok(())
            })?)
        };
        assert_eq!(actual.to_string(), expected.to_string());
    }

    #[test]
    fn fn_no_args_no_throw() {
        let item = parse_quote! {
            fn geti() -> i32 { 1 }
        };

        let expander = Expander::new();
        let path = Vec::new();

        let actual = expander.function(&path, &item);
        let expected = quote! {
            obj.set("geti", rquickjs::Function::new(ctx, "geti", |ctx: rquickjs::Ctx<'_>, this: rquickjs::Value<'_>, _: ()| -> rquickjs::Result<_> {
                Ok(geti())
            })?)
        };
        assert_eq!(actual.to_string(), expected.to_string());
    }

    #[test]
    fn fn_no_args_may_throw() {
        let item = parse_quote! {
            fn test() -> Result<()> { Ok(()) }
        };

        let expander = Expander::new();
        let path = Vec::new();

        let actual = expander.function(&path, &item);
        let expected = quote! {
            obj.set("test", rquickjs::Function::new(ctx, "test", |ctx: rquickjs::Ctx<'_>, this: rquickjs::Value<'_>, _: ()| -> rquickjs::Result<_> {
                test().map_err(|error| error.into())
            })?)
        };
        assert_eq!(actual.to_string(), expected.to_string());
    }

    #[test]
    fn fn_one_arg_no_throw() {
        let item = parse_quote! {
            fn incr(val: i32) -> i32 { val + 1 }
        };

        let expander = Expander::new();
        let path = Vec::new();

        let actual = expander.function(&path, &item);
        let expected = quote! {
            obj.set("incr", rquickjs::Function::new(ctx, "incr", |ctx: rquickjs::Ctx<'_>, this: rquickjs::Value<'_>, val: i32| -> rquickjs::Result<_> {
                Ok(incr(val))
            })?)
        };
        assert_eq!(actual.to_string(), expected.to_string());
    }

    #[test]
    fn fn_two_args_no_throw() {
        let item = parse_quote! {
            fn add2(a: i32, b: i32) -> i32 { a + b }
        };

        let expander = Expander::new();
        let path = Vec::new();

        let actual = expander.function(&path, &item);
        let expected = quote! {
            obj.set("add2", rquickjs::Function::new(ctx, "add2", |ctx: rquickjs::Ctx<'_>, this: rquickjs::Value<'_>, (a, b): (i32, i32)| -> rquickjs::Result<_> {
                Ok(add2(a, b))
            })?)
        };
        assert_eq!(actual.to_string(), expected.to_string());
    }

    #[test]
    fn async_fn_no_args_no_return() {
        let item = parse_quote! {
            async fn doit() {}
        };

        let expander = Expander::new();
        let path = Vec::new();

        let actual = expander.function(&path, &item);
        let expected = quote! {
            obj.set("doit", rquickjs::Function::new(ctx, "doit", |ctx: rquickjs::Ctx<'_>, this: rquickjs::Value<'_>, _: ()| -> rquickjs::Result<_> {
                Ok(rquickjs::PromiseJs(async move {
                    doit().await;
                    Ok(())
                }))
            })?)
        };
        assert_eq!(actual.to_string(), expected.to_string());
    }

    #[test]
    fn async_fn_no_args_may_throw() {
        let item = parse_quote! {
            async fn test() -> Result<()> { Ok(()) }
        };

        let expander = Expander::new();
        let path = Vec::new();

        let actual = expander.function(&path, &item);
        let expected = quote! {
            obj.set("test", rquickjs::Function::new(ctx, "test", |ctx: rquickjs::Ctx<'_>, this: rquickjs::Value<'_>, _: ()| -> rquickjs::Result<_> {
                Ok(rquickjs::PromiseJs(async move {
                    test().await.map_err(|error| error.into())
                }))
            })?)
        };
        assert_eq!(actual.to_string(), expected.to_string());
    }

    #[test]
    fn async_fn_two_args_no_throw() {
        let item = parse_quote! {
            async fn mul2(a: f64, b: f64) -> f64 {
                a * b
            }
        };

        let expander = Expander::new();
        let path = Vec::new();

        let actual = expander.function(&path, &item);
        let expected = quote! {
            obj.set("mul2", rquickjs::Function::new(ctx, "mul2", |ctx: rquickjs::Ctx<'_>, this: rquickjs::Value<'_>, (a, b): (f64, f64)| -> rquickjs::Result<_> {
                Ok(rquickjs::PromiseJs(async move {
                    Ok(mul2(a, b).await)
                }))
            })?)
        };
        assert_eq!(actual.to_string(), expected.to_string());
    }
}
